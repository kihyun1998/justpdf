//! ByteRange digest computation for PDF signatures.

use sha2::Digest;

use crate::error::{JustPdfError, Result};
use super::types::DigestAlgorithm;

/// Compute the digest over the signed byte ranges of a PDF.
///
/// `byte_range` is [offset1, length1, offset2, length2] from the signature dict.
/// The digest covers pdf_data[offset1..offset1+length1] + pdf_data[offset2..offset2+length2].
pub fn compute_byterange_digest(
    pdf_data: &[u8],
    byte_range: &[i64],
    algorithm: DigestAlgorithm,
) -> Result<Vec<u8>> {
    if byte_range.len() != 4 {
        return Err(JustPdfError::SignatureError {
            detail: format!("invalid ByteRange length: {}", byte_range.len()),
        });
    }

    let ranges = [
        (byte_range[0] as usize, byte_range[1] as usize),
        (byte_range[2] as usize, byte_range[3] as usize),
    ];

    // Validate ranges
    for (offset, length) in &ranges {
        if offset + length > pdf_data.len() {
            return Err(JustPdfError::SignatureError {
                detail: format!(
                    "ByteRange exceeds file size: offset={} length={} file_size={}",
                    offset, length, pdf_data.len()
                ),
            });
        }
    }

    match algorithm {
        DigestAlgorithm::Sha256 => {
            let mut hasher = sha2::Sha256::new();
            for (offset, length) in &ranges {
                hasher.update(&pdf_data[*offset..*offset + *length]);
            }
            Ok(hasher.finalize().to_vec())
        }
        DigestAlgorithm::Sha384 => {
            let mut hasher = sha2::Sha384::new();
            for (offset, length) in &ranges {
                hasher.update(&pdf_data[*offset..*offset + *length]);
            }
            Ok(hasher.finalize().to_vec())
        }
        DigestAlgorithm::Sha512 => {
            let mut hasher = sha2::Sha512::new();
            for (offset, length) in &ranges {
                hasher.update(&pdf_data[*offset..*offset + *length]);
            }
            Ok(hasher.finalize().to_vec())
        }
    }
}

/// Check whether the PDF has been modified after signing.
///
/// If the signed byte range doesn't extend to the end of the file,
/// there are extra bytes appended after signing.
pub fn detect_modification_after_signing(
    pdf_data: &[u8],
    byte_range: &[i64],
) -> bool {
    if byte_range.len() != 4 {
        return false;
    }

    let last_offset = byte_range[2] as usize;
    let last_length = byte_range[3] as usize;
    let signed_end = last_offset + last_length;

    // If there are bytes beyond the signed range, the file was modified
    signed_end < pdf_data.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_digest_sha256() {
        // 80 bytes total: [0..36] + skip [36..57] + [57..80]
        let data = b"Hello, World! This is signed content.SIGNATURE_PLACEHOLDERMore signed content!!";
        assert_eq!(data.len(), 79);
        let byte_range = vec![0i64, 36, 57, 22];
        let digest = compute_byterange_digest(data, &byte_range, DigestAlgorithm::Sha256).unwrap();
        assert_eq!(digest.len(), 32);

        // Same input => same digest
        let digest2 = compute_byterange_digest(data, &byte_range, DigestAlgorithm::Sha256).unwrap();
        assert_eq!(digest, digest2);
    }

    #[test]
    fn test_compute_digest_sha384() {
        let data = b"AAAAABBBBBCCCCCDDDDDEEEEE"; // 25 bytes
        let byte_range = vec![0i64, 10, 15, 10];
        let digest = compute_byterange_digest(data, &byte_range, DigestAlgorithm::Sha384).unwrap();
        assert_eq!(digest.len(), 48);
    }

    #[test]
    fn test_compute_digest_sha512() {
        let data = b"AAAAABBBBBCCCCCDDDDDEEEEE"; // 25 bytes
        let byte_range = vec![0i64, 10, 15, 10];
        let digest = compute_byterange_digest(data, &byte_range, DigestAlgorithm::Sha512).unwrap();
        assert_eq!(digest.len(), 64);
    }

    #[test]
    fn test_invalid_byte_range() {
        let data = b"short";
        let byte_range = vec![0i64, 100, 200, 300]; // exceeds data
        assert!(compute_byterange_digest(data, &byte_range, DigestAlgorithm::Sha256).is_err());
    }

    #[test]
    fn test_detect_modification() {
        // File is 100 bytes, signed range covers [0,40]+[60,40] = ends at 100
        assert!(!detect_modification_after_signing(&[0u8; 100], &[0, 40, 60, 40]));

        // File is 120 bytes, signed range ends at 100 => modified
        assert!(detect_modification_after_signing(&[0u8; 120], &[0, 40, 60, 40]));
    }
}
