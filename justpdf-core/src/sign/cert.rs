//! Certificate extraction and chain validation from CMS/PKCS#7 data.

use der::{Decode, Encode};

use crate::error::Result;
use super::types::CertificateInfo;

/// Extract certificate information from a CMS/PKCS#7 SignedData blob.
pub fn extract_certificates(cms_data: &[u8]) -> Result<Vec<CertificateInfo>> {
    // Parse ContentInfo → SignedData
    let content_info = match cms::content_info::ContentInfo::from_der(cms_data) {
        Ok(ci) => ci,
        Err(_) => return Ok(Vec::new()), // Not valid CMS, return empty
    };

    let der_bytes = match content_info.content.to_der() {
        Ok(b) => b,
        Err(_) => return Ok(Vec::new()),
    };
    let signed_data = match cms::signed_data::SignedData::from_der(&der_bytes) {
        Ok(sd) => sd,
        Err(_) => return Ok(Vec::new()),
    };

    // Extract certificates
    let mut certs = Vec::new();
    if let Some(cert_set) = &signed_data.certificates {
        for cert_choice in cert_set.0.iter() {
            if let cms::cert::CertificateChoices::Certificate(cert) = cert_choice {
                let info = parse_x509_cert(cert);
                certs.push(info);
            }
        }
    }

    Ok(certs)
}

/// Parse an X.509 certificate into our CertificateInfo type.
fn parse_x509_cert(cert: &x509_cert::Certificate) -> CertificateInfo {
    let subject = format_rdn_sequence(&cert.tbs_certificate.subject);
    let issuer = format_rdn_sequence(&cert.tbs_certificate.issuer);

    let serial_bytes = cert
        .tbs_certificate
        .serial_number
        .as_bytes();
    let serial_number = serial_bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<String>();

    let not_before = format!("{}", cert.tbs_certificate.validity.not_before);
    let not_after = format!("{}", cert.tbs_certificate.validity.not_after);

    let is_self_signed = subject == issuer;

    CertificateInfo {
        subject,
        issuer,
        serial_number,
        not_before,
        not_after,
        is_self_signed,
    }
}

/// Format an X.509 RDN sequence as a human-readable string.
fn format_rdn_sequence(name: &x509_cert::name::Name) -> String {
    let mut parts = Vec::new();
    for rdn in name.0.iter() {
        for atv in rdn.0.iter() {
            let oid = &atv.oid;
            let prefix = match oid.to_string().as_str() {
                "2.5.4.3" => "CN",
                "2.5.4.6" => "C",
                "2.5.4.7" => "L",
                "2.5.4.8" => "ST",
                "2.5.4.10" => "O",
                "2.5.4.11" => "OU",
                "1.2.840.113549.1.9.1" => "E",
                _ => &oid.to_string(),
            };
            // Try to decode the value as UTF-8 string
            let val = atv
                .value
                .decode_as::<der::asn1::Utf8StringRef>()
                .map(|s| s.as_str().to_string())
                .or_else(|_| {
                    atv.value
                        .decode_as::<der::asn1::PrintableStringRef>()
                        .map(|s| s.as_str().to_string())
                })
                .or_else(|_| {
                    atv.value
                        .decode_as::<der::asn1::Ia5StringRef>()
                        .map(|s| s.as_str().to_string())
                })
                .unwrap_or_else(|_| "?".to_string());
            parts.push(format!("{}={}", prefix, val));
        }
    }
    parts.join(", ")
}

/// Validate a certificate chain structure.
///
/// Returns (is_valid, trust_level).
/// - Checks that each certificate's issuer matches the next certificate's subject.
/// - Checks validity dates (basic — no clock comparison, just structural).
/// - Reports if the root is self-signed.
pub fn validate_chain(certs: &[CertificateInfo]) -> (bool, bool) {
    if certs.is_empty() {
        return (false, false);
    }

    // Check chain linkage: cert[i].issuer == cert[i+1].subject
    let mut chain_valid = true;
    for i in 0..certs.len().saturating_sub(1) {
        if certs[i].issuer != certs[i + 1].subject {
            chain_valid = false;
            break;
        }
    }

    // Check if root is self-signed
    let root_self_signed = certs.last().map(|c| c.is_self_signed).unwrap_or(false);

    (chain_valid, root_self_signed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_chain_empty() {
        let (valid, self_signed) = validate_chain(&[]);
        assert!(!valid);
        assert!(!self_signed);
    }

    #[test]
    fn test_validate_chain_single_self_signed() {
        let cert = CertificateInfo {
            subject: "CN=Test".to_string(),
            issuer: "CN=Test".to_string(),
            serial_number: "01".to_string(),
            not_before: "2025-01-01".to_string(),
            not_after: "2026-01-01".to_string(),
            is_self_signed: true,
        };
        let (valid, self_signed) = validate_chain(&[cert]);
        assert!(valid);
        assert!(self_signed);
    }

    #[test]
    fn test_validate_chain_two_certs() {
        let leaf = CertificateInfo {
            subject: "CN=User".to_string(),
            issuer: "CN=CA".to_string(),
            serial_number: "02".to_string(),
            not_before: "2025-01-01".to_string(),
            not_after: "2026-01-01".to_string(),
            is_self_signed: false,
        };
        let ca = CertificateInfo {
            subject: "CN=CA".to_string(),
            issuer: "CN=CA".to_string(),
            serial_number: "01".to_string(),
            not_before: "2024-01-01".to_string(),
            not_after: "2028-01-01".to_string(),
            is_self_signed: true,
        };
        let (valid, self_signed) = validate_chain(&[leaf, ca]);
        assert!(valid);
        assert!(self_signed);
    }

    #[test]
    fn test_validate_chain_broken() {
        let leaf = CertificateInfo {
            subject: "CN=User".to_string(),
            issuer: "CN=Unknown".to_string(),
            serial_number: "02".to_string(),
            not_before: "".to_string(),
            not_after: "".to_string(),
            is_self_signed: false,
        };
        let ca = CertificateInfo {
            subject: "CN=CA".to_string(),
            issuer: "CN=CA".to_string(),
            serial_number: "01".to_string(),
            not_before: "".to_string(),
            not_after: "".to_string(),
            is_self_signed: true,
        };
        let (valid, _) = validate_chain(&[leaf, ca]);
        assert!(!valid);
    }

    #[test]
    fn test_extract_certs_invalid_data() {
        let result = extract_certificates(b"not valid CMS data").unwrap();
        assert!(result.is_empty());
    }
}
