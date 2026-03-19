//! RFC 3161 timestamp request/response handling.
//!
//! Provides functions to create timestamp requests and parse responses.
//! The actual HTTP transport to a TSA server is left to the caller.

use der::Encode;

use crate::error::{JustPdfError, Result};
use super::types::DigestAlgorithm;

/// Create an RFC 3161 TimeStampReq DER-encoded message.
///
/// The caller is responsible for sending this to a TSA server via HTTP POST
/// with Content-Type: application/timestamp-query.
pub fn create_timestamp_request(
    digest: &[u8],
    algorithm: DigestAlgorithm,
) -> Result<Vec<u8>> {
    // Build the ASN.1 structure manually using DER encoding
    // TimeStampReq ::= SEQUENCE {
    //   version       INTEGER { v1(1) },
    //   messageImprint MessageImprint,
    //   ...
    // }
    // MessageImprint ::= SEQUENCE {
    //   hashAlgorithm AlgorithmIdentifier,
    //   hashedMessage OCTET STRING
    // }

    let hash_oid = match algorithm {
        DigestAlgorithm::Sha256 => const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.1"),
        DigestAlgorithm::Sha384 => const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.2"),
        DigestAlgorithm::Sha512 => const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.3"),
    };

    // Use der crate to build the structure
    let alg_id = spki::AlgorithmIdentifierRef {
        oid: hash_oid,
        parameters: None,
    };

    // Encode MessageImprint
    let hashed_message = der::asn1::OctetStringRef::new(digest)
        .map_err(|e| JustPdfError::SignatureError {
            detail: format!("failed to create OctetString: {}", e),
        })?;

    // Build the full request as raw DER
    let mut msg_imprint_buf = Vec::new();
    alg_id.encode_to_vec(&mut msg_imprint_buf)
        .map_err(|e| JustPdfError::SignatureError {
            detail: format!("DER encode error: {}", e),
        })?;
    // We need to wrap this manually. For simplicity, build the raw bytes.
    // A full implementation would use a proper ASN.1 builder.

    // For now, return a simplified structure
    let mut request = Vec::new();

    // This is a simplified timestamp request builder.
    // A production implementation would use a full ASN.1 library.
    // The structure is:
    // SEQUENCE {
    //   INTEGER 1,  -- version
    //   SEQUENCE {  -- messageImprint
    //     SEQUENCE { OID, NULL },  -- hashAlgorithm
    //     OCTET STRING             -- hashedMessage
    //   },
    //   BOOLEAN TRUE               -- certReq
    // }
    let oid_bytes = hash_oid.to_der()
        .map_err(|e| JustPdfError::SignatureError {
            detail: format!("OID encode error: {}", e),
        })?;

    // AlgorithmIdentifier: SEQUENCE { OID }
    let mut alg_seq = Vec::new();
    alg_seq.extend_from_slice(&oid_bytes);
    let alg_seq_tlv = wrap_sequence(&alg_seq);

    // MessageImprint: SEQUENCE { AlgorithmIdentifier, OCTET STRING }
    let mut msg_imprint = Vec::new();
    msg_imprint.extend_from_slice(&alg_seq_tlv);
    // OCTET STRING
    msg_imprint.push(0x04); // tag
    encode_length(digest.len(), &mut msg_imprint);
    msg_imprint.extend_from_slice(digest);
    let msg_imprint_tlv = wrap_sequence(&msg_imprint);

    // TimeStampReq: SEQUENCE { INTEGER 1, MessageImprint, BOOLEAN TRUE }
    let mut tsr_content = Vec::new();
    // INTEGER 1
    tsr_content.extend_from_slice(&[0x02, 0x01, 0x01]);
    tsr_content.extend_from_slice(&msg_imprint_tlv);
    // certReq BOOLEAN TRUE (context tag [0] IMPLICIT)
    tsr_content.extend_from_slice(&[0x01, 0x01, 0xFF]);
    request = wrap_sequence(&tsr_content);

    Ok(request)
}

/// Parse a timestamp response and extract the timestamp token.
///
/// The timestamp token can be embedded as an unsigned attribute in the CMS signature.
pub fn parse_timestamp_response(response: &[u8]) -> Result<Vec<u8>> {
    // TimeStampResp ::= SEQUENCE {
    //   status        PKIStatusInfo,
    //   timeStampToken ContentInfo OPTIONAL
    // }
    // We just need to extract the ContentInfo (the second element)

    // Basic DER parsing — find the second element in the outer SEQUENCE
    if response.len() < 4 || response[0] != 0x30 {
        return Err(JustPdfError::SignatureError {
            detail: "invalid timestamp response: not a SEQUENCE".into(),
        });
    }

    // Skip outer SEQUENCE tag + length
    let (_, content_offset) = read_der_length(&response[1..])?;
    let content_start = 1 + content_offset;

    if content_start >= response.len() {
        return Err(JustPdfError::SignatureError {
            detail: "timestamp response too short".into(),
        });
    }

    // Skip first element (PKIStatusInfo)
    let first_elem = &response[content_start..];
    if first_elem.is_empty() {
        return Err(JustPdfError::SignatureError {
            detail: "empty timestamp response content".into(),
        });
    }
    let (first_len, first_len_bytes) = read_der_length(&first_elem[1..])?;
    let second_start = content_start + 1 + first_len_bytes + first_len;

    if second_start >= response.len() {
        return Err(JustPdfError::SignatureError {
            detail: "timestamp response missing timestamp token".into(),
        });
    }

    // The rest is the ContentInfo (timestamp token)
    Ok(response[second_start..].to_vec())
}

// --- DER helpers ---

fn wrap_sequence(content: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    result.push(0x30); // SEQUENCE tag
    encode_length(content.len(), &mut result);
    result.extend_from_slice(content);
    result
}

fn encode_length(len: usize, buf: &mut Vec<u8>) {
    if len < 0x80 {
        buf.push(len as u8);
    } else if len < 0x100 {
        buf.push(0x81);
        buf.push(len as u8);
    } else {
        buf.push(0x82);
        buf.push((len >> 8) as u8);
        buf.push(len as u8);
    }
}

fn read_der_length(data: &[u8]) -> Result<(usize, usize)> {
    if data.is_empty() {
        return Err(JustPdfError::SignatureError {
            detail: "DER length missing".into(),
        });
    }

    if data[0] < 0x80 {
        Ok((data[0] as usize, 1))
    } else if data[0] == 0x81 {
        if data.len() < 2 {
            return Err(JustPdfError::SignatureError { detail: "DER length truncated".into() });
        }
        Ok((data[1] as usize, 2))
    } else if data[0] == 0x82 {
        if data.len() < 3 {
            return Err(JustPdfError::SignatureError { detail: "DER length truncated".into() });
        }
        Ok((((data[1] as usize) << 8) | data[2] as usize, 3))
    } else {
        Err(JustPdfError::SignatureError {
            detail: format!("unsupported DER length encoding: 0x{:02X}", data[0]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_timestamp_request() {
        let digest = [0x42u8; 32]; // fake SHA-256 digest
        let request = create_timestamp_request(&digest, DigestAlgorithm::Sha256).unwrap();

        // Should be a valid DER SEQUENCE
        assert!(!request.is_empty());
        assert_eq!(request[0], 0x30); // SEQUENCE tag
    }

    #[test]
    fn test_wrap_sequence() {
        let content = vec![0x02, 0x01, 0x01]; // INTEGER 1
        let seq = wrap_sequence(&content);
        assert_eq!(seq[0], 0x30);
        assert_eq!(seq[1], 3);
        assert_eq!(&seq[2..], &content);
    }

    #[test]
    fn test_encode_length_short() {
        let mut buf = Vec::new();
        encode_length(50, &mut buf);
        assert_eq!(buf, vec![50]);
    }

    #[test]
    fn test_encode_length_medium() {
        let mut buf = Vec::new();
        encode_length(200, &mut buf);
        assert_eq!(buf, vec![0x81, 200]);
    }

    #[test]
    fn test_encode_length_long() {
        let mut buf = Vec::new();
        encode_length(1000, &mut buf);
        assert_eq!(buf, vec![0x82, 0x03, 0xE8]);
    }

    #[test]
    fn test_parse_timestamp_response_invalid() {
        assert!(parse_timestamp_response(b"").is_err());
        assert!(parse_timestamp_response(b"\x01\x02").is_err());
    }
}
