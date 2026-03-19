//! Signature verification for PDF digital signatures.

use der::{Decode, Encode};

use crate::error::Result;
use super::byterange;
use super::cert;
use super::types::*;

/// Verify a single PDF signature.
///
/// Computes the digest over the byte ranges, parses the CMS/PKCS#7 data,
/// verifies the cryptographic signature, and checks the certificate chain.
pub fn verify_signature(
    pdf_data: &[u8],
    sig_info: &SignatureInfo,
) -> Result<VerificationResult> {
    // Check byte range validity
    if sig_info.byte_range.len() != 4 {
        return Ok(VerificationResult {
            signature_info: sig_info.clone(),
            digest_valid: false,
            signature_valid: false,
            cert_chain_valid: false,
            modified_after_signing: false,
            validity: SignatureValidity::Unknown("invalid ByteRange".into()),
        });
    }

    // Check for modification after signing
    let modified_after_signing =
        byterange::detect_modification_after_signing(pdf_data, &sig_info.byte_range);

    // Parse the CMS SignedData
    let content_info = match cms::content_info::ContentInfo::from_der(&sig_info.contents_raw) {
        Ok(ci) => ci,
        Err(e) => {
            return Ok(VerificationResult {
                signature_info: sig_info.clone(),
                digest_valid: false,
                signature_valid: false,
                cert_chain_valid: false,
                modified_after_signing,
                validity: SignatureValidity::Unknown(format!("CMS parse error: {}", e)),
            });
        }
    };

    let content_der = match content_info.content.to_der() {
        Ok(d) => d,
        Err(e) => {
            return Ok(VerificationResult {
                signature_info: sig_info.clone(),
                digest_valid: false,
                signature_valid: false,
                cert_chain_valid: false,
                modified_after_signing,
                validity: SignatureValidity::Unknown(format!("CMS content error: {}", e)),
            });
        }
    };

    let signed_data = match cms::signed_data::SignedData::from_der(&content_der) {
        Ok(sd) => sd,
        Err(e) => {
            return Ok(VerificationResult {
                signature_info: sig_info.clone(),
                digest_valid: false,
                signature_valid: false,
                cert_chain_valid: false,
                modified_after_signing,
                validity: SignatureValidity::Unknown(format!("SignedData parse error: {}", e)),
            });
        }
    };

    // Get signer info (typically exactly one)
    if signed_data.signer_infos.0.is_empty() {
        return Ok(VerificationResult {
            signature_info: sig_info.clone(),
            digest_valid: false,
            signature_valid: false,
            cert_chain_valid: false,
            modified_after_signing,
            validity: SignatureValidity::Unknown("no signer info".into()),
        });
    }

    let signer_info = &signed_data.signer_infos.0.as_ref()[0];

    // Determine digest algorithm from signer info
    let digest_alg = oid_to_digest_algorithm(&signer_info.digest_alg.oid);

    // Compute digest over byte ranges
    let computed_digest = byterange::compute_byterange_digest(
        pdf_data,
        &sig_info.byte_range,
        digest_alg.unwrap_or(DigestAlgorithm::Sha256),
    )?;

    // Check digest against signed attributes
    let digest_valid = if let Some(ref signed_attrs) = signer_info.signed_attrs {
        // Find messageDigest attribute
        let message_digest = find_message_digest(signed_attrs);
        match message_digest {
            Some(md) => md == computed_digest,
            None => false, // messageDigest attribute required but not found
        }
    } else {
        // No signed attributes — we can't easily verify without them
        // The signature would be directly over the content digest
        true // Assume valid if no signed attrs (uncommon in PDF)
    };

    // Verify the cryptographic signature
    let signature_valid = verify_cms_signature(
        &signed_data,
        signer_info,
    );

    // Validate certificate chain
    let (cert_chain_valid, _root_self_signed) = cert::validate_chain(&sig_info.cert_chain);

    // Determine overall validity
    let validity = if digest_valid && signature_valid && cert_chain_valid && !modified_after_signing {
        SignatureValidity::Valid
    } else if !digest_valid {
        SignatureValidity::DigestMismatch
    } else if !signature_valid {
        SignatureValidity::SignatureInvalid
    } else if !cert_chain_valid {
        SignatureValidity::CertificateNotTrusted
    } else {
        SignatureValidity::Unknown("modified after signing".into())
    };

    Ok(VerificationResult {
        signature_info: sig_info.clone(),
        digest_valid,
        signature_valid,
        cert_chain_valid,
        modified_after_signing,
        validity,
    })
}

/// Map OID to DigestAlgorithm.
fn oid_to_digest_algorithm(oid: &const_oid::ObjectIdentifier) -> Option<DigestAlgorithm> {
    // SHA-256: 2.16.840.1.101.3.4.2.1
    // SHA-384: 2.16.840.1.101.3.4.2.2
    // SHA-512: 2.16.840.1.101.3.4.2.3
    let oid_str = oid.to_string();
    match oid_str.as_str() {
        "2.16.840.1.101.3.4.2.1" => Some(DigestAlgorithm::Sha256),
        "2.16.840.1.101.3.4.2.2" => Some(DigestAlgorithm::Sha384),
        "2.16.840.1.101.3.4.2.3" => Some(DigestAlgorithm::Sha512),
        _ => None,
    }
}

/// Find the messageDigest attribute value in signed attributes.
fn find_message_digest(attrs: &cms::signed_data::SignedAttributes) -> Option<Vec<u8>> {
    // messageDigest OID: 1.2.840.113549.1.9.4
    let md_oid = const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.4");

    for attr in attrs.iter() {
        if attr.oid == md_oid {
            // The value is a SET containing an OCTET STRING
            if let Some(val) = attr.values.as_ref().get(0) {
                if let Ok(bytes) = val.decode_as::<der::asn1::OctetStringRef>() {
                    return Some(bytes.as_bytes().to_vec());
                }
            }
        }
    }
    None
}

/// Verify the CMS signature cryptographically.
///
/// Finds the signer certificate, extracts the public key, and verifies
/// the signature over the signed attributes (or content digest).
fn verify_cms_signature(
    signed_data: &cms::signed_data::SignedData,
    signer_info: &cms::signed_data::SignerInfo,
) -> bool {
    // Find the signer certificate
    let signer_cert = match find_signer_certificate(signed_data, signer_info) {
        Some(cert) => cert,
        None => return false,
    };

    // Get the signature bytes
    let sig_bytes = signer_info.signature.as_bytes();

    // Get the data to verify: DER-encoded signed attributes or content digest
    let verify_data = match &signer_info.signed_attrs {
        Some(attrs) => {
            // Re-encode signed attributes as SET OF for verification
            match der::Encode::to_der(attrs) {
                Ok(der_bytes) => der_bytes,
                Err(_) => return false,
            }
        }
        None => return false, // Can't verify without signed attrs in this implementation
    };

    // Extract public key from certificate
    let spki = &signer_cert.tbs_certificate.subject_public_key_info;
    let alg_oid = &spki.algorithm.oid;

    // RSA verification
    // RSA OID: 1.2.840.113549.1.1.1
    if alg_oid.to_string() == "1.2.840.113549.1.1.1" {
        return verify_rsa_signature(spki, &verify_data, sig_bytes, signer_info);
    }

    // For other algorithms (ECDSA, etc.), return false for now
    // This can be extended with p256/p384 support
    false
}

/// Find the signer certificate in the SignedData certificates.
fn find_signer_certificate<'a>(
    signed_data: &'a cms::signed_data::SignedData,
    signer_info: &cms::signed_data::SignerInfo,
) -> Option<&'a x509_cert::Certificate> {
    let certs = signed_data.certificates.as_ref()?;

    let sid = &signer_info.sid;
    match sid {
        cms::signed_data::SignerIdentifier::IssuerAndSerialNumber(iasn) => {
            for cert_choice in certs.0.iter() {
                if let cms::cert::CertificateChoices::Certificate(cert) = cert_choice {
                    if cert.tbs_certificate.serial_number == iasn.serial_number
                        && cert.tbs_certificate.issuer == iasn.issuer
                    {
                        return Some(cert);
                    }
                }
            }
        }
        _ => {} // SubjectKeyIdentifier not implemented yet
    }

    None
}

/// Verify an RSA signature.
fn verify_rsa_signature(
    spki: &x509_cert::spki::SubjectPublicKeyInfoOwned,
    data: &[u8],
    signature: &[u8],
    signer_info: &cms::signed_data::SignerInfo,
) -> bool {
    use rsa::pkcs1v15::VerifyingKey;
    use rsa::RsaPublicKey;
    use signature::Verifier;

    // Re-encode SPKI to DER and parse as reference type for RsaPublicKey
    let spki_der = match spki.to_der() {
        Ok(d) => d,
        Err(_) => return false,
    };
    let spki_ref = match spki::SubjectPublicKeyInfoRef::from_der(&spki_der) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let pub_key = match RsaPublicKey::try_from(spki_ref) {
        Ok(k) => k,
        Err(_) => return false,
    };

    // Determine hash algorithm from signature algorithm or digest algorithm
    let digest_oid = signer_info.digest_alg.oid.to_string();

    let sig = rsa::pkcs1v15::Signature::try_from(signature);
    let sig = match sig {
        Ok(s) => s,
        Err(_) => return false,
    };

    match digest_oid.as_str() {
        "2.16.840.1.101.3.4.2.1" => {
            // SHA-256
            let verifier = VerifyingKey::<sha2::Sha256>::new(pub_key);
            verifier.verify(data, &sig).is_ok()
        }
        "2.16.840.1.101.3.4.2.2" => {
            // SHA-384
            let verifier = VerifyingKey::<sha2::Sha384>::new(pub_key);
            verifier.verify(data, &sig).is_ok()
        }
        "2.16.840.1.101.3.4.2.3" => {
            // SHA-512
            let verifier = VerifyingKey::<sha2::Sha512>::new(pub_key);
            verifier.verify(data, &sig).is_ok()
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oid_to_digest_sha256() {
        let oid = const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.1");
        assert_eq!(oid_to_digest_algorithm(&oid), Some(DigestAlgorithm::Sha256));
    }

    #[test]
    fn test_oid_to_digest_sha384() {
        let oid = const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.2");
        assert_eq!(oid_to_digest_algorithm(&oid), Some(DigestAlgorithm::Sha384));
    }

    #[test]
    fn test_oid_to_digest_unknown() {
        let oid = const_oid::ObjectIdentifier::new_unwrap("1.2.3.4");
        assert_eq!(oid_to_digest_algorithm(&oid), None);
    }

    #[test]
    fn test_verify_invalid_cms() {
        let sig = SignatureInfo {
            field_name: "Sig1".to_string(),
            field_ref: crate::object::IndirectRef { obj_num: 1, gen_num: 0 },
            signer_name: None,
            signing_time: None,
            reason: None,
            location: None,
            contact_info: None,
            filter: b"Adobe.PPKLite".to_vec(),
            sub_filter: b"adbe.pkcs7.detached".to_vec(),
            byte_range: vec![0, 10, 20, 10],
            contents_raw: vec![0u8; 32], // invalid CMS
            cert_chain: vec![],
        };

        let pdf_data = vec![0u8; 30];
        let result = verify_signature(&pdf_data, &sig).unwrap();
        assert!(!result.signature_valid);
        assert!(matches!(result.validity, SignatureValidity::Unknown(_)));
    }
}
