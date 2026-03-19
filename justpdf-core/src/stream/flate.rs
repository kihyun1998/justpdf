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

/// Attempt to decode FlateDecode data, returning whatever was successfully
/// decompressed even if the stream is truncated or corrupted partway through.
/// Returns `Ok(data)` with partial data on soft failures, `Err` only if
/// absolutely nothing could be decoded.
pub fn decode_partial(data: &[u8]) -> Result<Vec<u8>> {
    // Try normal decode first
    if let Ok(result) = decode(data) {
        return Ok(result);
    }

    // Try zlib partial first (most PDF streams use zlib headers).
    // Only fall back to raw deflate if zlib produced nothing, since raw
    // deflate can misinterpret the zlib header bytes and produce garbage.
    if let Ok(result) = decode_partial_zlib(data) {
        return Ok(result);
    }

    if let Ok(result) = decode_partial_raw(data) {
        return Ok(result);
    }

    Err(JustPdfError::StreamDecode {
        filter: "FlateDecode".into(),
        detail: "could not recover any data from broken stream".into(),
    })
}

/// Read as much as possible from a zlib stream, ignoring trailing errors.
fn decode_partial_zlib(data: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(data);
    let mut result = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match decoder.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => result.extend_from_slice(&buf[..n]),
            Err(_) => break, // stop on error, keep what we have
        }
    }
    if result.is_empty() {
        Err(JustPdfError::StreamDecode {
            filter: "FlateDecode".into(),
            detail: "zlib partial decode produced no data".into(),
        })
    } else {
        Ok(result)
    }
}

/// Read as much as possible from a raw deflate stream, ignoring trailing errors.
fn decode_partial_raw(data: &[u8]) -> Result<Vec<u8>> {
    use flate2::read::DeflateDecoder;
    let mut decoder = DeflateDecoder::new(data);
    let mut result = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match decoder.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => result.extend_from_slice(&buf[..n]),
            Err(_) => break,
        }
    }
    if result.is_empty() {
        Err(JustPdfError::StreamDecode {
            filter: "FlateDecode".into(),
            detail: "raw deflate partial decode produced no data".into(),
        })
    } else {
        Ok(result)
    }
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
