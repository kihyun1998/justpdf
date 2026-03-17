use flate2::read::ZlibDecoder;
use std::io::Read;

use crate::error::{JustPdfError, Result};

/// Decode FlateDecode (zlib/deflate) compressed data.
pub fn decode(data: &[u8]) -> Result<Vec<u8>> {
    // Try zlib first (with header)
    if let Ok(result) = decode_zlib(data) {
        return Ok(result);
    }

    // Fallback: try raw deflate (no zlib header)
    decode_raw_deflate(data)
}

fn decode_zlib(data: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(data);
    let mut result = Vec::new();
    decoder
        .read_to_end(&mut result)
        .map_err(|e| JustPdfError::StreamDecode {
            filter: "FlateDecode".into(),
            detail: format!("zlib decode error: {e}"),
        })?;
    Ok(result)
}

fn decode_raw_deflate(data: &[u8]) -> Result<Vec<u8>> {
    use flate2::read::DeflateDecoder;
    let mut decoder = DeflateDecoder::new(data);
    let mut result = Vec::new();
    decoder
        .read_to_end(&mut result)
        .map_err(|e| JustPdfError::StreamDecode {
            filter: "FlateDecode".into(),
            detail: format!("deflate decode error: {e}"),
        })?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;

    #[test]
    fn test_flate_roundtrip() {
        let original = b"Hello, World! This is a test of FlateDecode compression.";

        // Compress
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(original).unwrap();
        let compressed = encoder.finish().unwrap();

        // Decompress
        let decoded = decode(&compressed).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_flate_empty() {
        let original = b"";
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(original).unwrap();
        let compressed = encoder.finish().unwrap();

        let decoded = decode(&compressed).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_flate_corrupted() {
        let result = decode(b"\x00\x01\x02\x03\x04");
        assert!(result.is_err());
    }
}
