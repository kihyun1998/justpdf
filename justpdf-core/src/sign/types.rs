//! Types for digital signature handling.

use crate::object::IndirectRef;

/// Digest algorithm used in a signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigestAlgorithm {
    Sha256,
    Sha384,
    Sha512,
}

/// Validity status of a signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureValidity {
    /// Signature is fully valid.
    Valid,
    /// Digest over byte range does not match.
    DigestMismatch,
    /// Cryptographic signature verification failed.
    SignatureInvalid,
    /// Signer certificate has expired.
    CertificateExpired,
    /// Certificate chain could not be verified to a trusted root.
    CertificateNotTrusted,
    /// No signatures found in the document.
    NoSignatures,
    /// Unknown or unsupported state.
    Unknown(String),
}

/// Information about a single digital signature in the PDF.
#[derive(Debug, Clone)]
pub struct SignatureInfo {
    /// Field name (from /T).
    pub field_name: String,
    /// Reference to the signature field object.
    pub field_ref: IndirectRef,
    /// Signer name (from /Name in sig dict).
    pub signer_name: Option<String>,
    /// Signing time (from /M in sig dict, PDF date string).
    pub signing_time: Option<String>,
    /// Reason for signing (from /Reason).
    pub reason: Option<String>,
    /// Location of signing (from /Location).
    pub location: Option<String>,
    /// Contact info (from /ContactInfo).
    pub contact_info: Option<String>,
    /// Filter name (e.g. "Adobe.PPKLite").
    pub filter: Vec<u8>,
    /// Sub-filter name (e.g. "adbe.pkcs7.detached").
    pub sub_filter: Vec<u8>,
    /// Byte range array [offset1, len1, offset2, len2].
    pub byte_range: Vec<i64>,
    /// Raw PKCS#7/CMS DER bytes from /Contents.
    pub contents_raw: Vec<u8>,
    /// Parsed certificate chain info.
    pub cert_chain: Vec<CertificateInfo>,
}

/// Certificate information extracted from X.509.
#[derive(Debug, Clone)]
pub struct CertificateInfo {
    /// Subject distinguished name.
    pub subject: String,
    /// Issuer distinguished name.
    pub issuer: String,
    /// Serial number (hex string).
    pub serial_number: String,
    /// Not-before date.
    pub not_before: String,
    /// Not-after date.
    pub not_after: String,
    /// Whether the certificate is self-signed (subject == issuer).
    pub is_self_signed: bool,
}

/// Complete verification result for one signature.
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// The signature info.
    pub signature_info: SignatureInfo,
    /// Whether the digest over the byte range matches.
    pub digest_valid: bool,
    /// Whether the cryptographic signature is valid.
    pub signature_valid: bool,
    /// Whether the certificate chain is valid.
    pub cert_chain_valid: bool,
    /// Whether the PDF was modified after signing.
    pub modified_after_signing: bool,
    /// Overall validity.
    pub validity: SignatureValidity,
}

/// Options for creating a PDF signature.
#[derive(Debug, Clone)]
pub struct SigningOptions {
    /// Signer name to embed in the signature.
    pub signer_name: Option<String>,
    /// Reason for signing.
    pub reason: Option<String>,
    /// Location of signing.
    pub location: Option<String>,
    /// Contact information.
    pub contact_info: Option<String>,
    /// Digest algorithm (default: SHA-256).
    pub digest_algorithm: DigestAlgorithm,
}

impl Default for SigningOptions {
    fn default() -> Self {
        Self {
            signer_name: None,
            reason: None,
            location: None,
            contact_info: None,
            digest_algorithm: DigestAlgorithm::Sha256,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_signing_options() {
        let opts = SigningOptions::default();
        assert_eq!(opts.digest_algorithm, DigestAlgorithm::Sha256);
        assert!(opts.signer_name.is_none());
    }

    #[test]
    fn test_signature_validity_eq() {
        assert_eq!(SignatureValidity::Valid, SignatureValidity::Valid);
        assert_ne!(SignatureValidity::Valid, SignatureValidity::DigestMismatch);
    }
}
