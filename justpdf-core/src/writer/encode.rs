use flate2::Compression;
use flate2::write::ZlibEncoder;
use std::io::Write;

use crate::error::Result;
use crate::object::{PdfDict, PdfObject};

/// Compress data using zlib (FlateDecode).
pub fn encode_flate(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    let compressed = encoder.finish()?;
    Ok(compressed)
}

/// Create a stream dictionary and encoded data.
///
/// If `compress` is true, the data is compressed with FlateDecode and the
/// `/Filter` entry is set. The returned dict always contains `/Length`.
///
/// Returns `(dict, encoded_data)`.
pub fn make_stream(data: &[u8], compress: bool) -> (PdfDict, Vec<u8>) {
    if compress {
        let compressed = encode_flate(data).unwrap_or_else(|_| data.to_vec());
        let mut dict = PdfDict::new();
        dict.insert(
            b"Length".to_vec(),
            PdfObject::Integer(compressed.len() as i64),
        );
        dict.insert(
            b"Filter".to_vec(),
            PdfObject::Name(b"FlateDecode".to_vec()),
        );
        (dict, compressed)
    } else {
        let mut dict = PdfDict::new();
        dict.insert(
            b"Length".to_vec(),
            PdfObject::Integer(data.len() as i64),
        );
        (dict, data.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    #[test]
    fn test_flate_roundtrip() {
        let original = b"Hello, World! This is a test of PDF stream encoding.";
        let compressed = encode_flate(original).unwrap();
        assert_ne!(compressed, original);

        let mut decoder = ZlibDecoder::new(&compressed[..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).unwrap();
        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_flate_empty() {
        let compressed = encode_flate(b"").unwrap();
        let mut decoder = ZlibDecoder::new(&compressed[..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).unwrap();
        assert_eq!(decompressed, b"");
    }

    #[test]
    fn test_make_stream_uncompressed() {
        let data = b"raw content";
        let (dict, out_data) = make_stream(data, false);

        assert_eq!(out_data, data);
        assert_eq!(dict.get_i64(b"Length"), Some(data.len() as i64));
        assert!(dict.get(b"Filter").is_none());
    }

    #[test]
    fn test_make_stream_compressed() {
        let data = b"some content to compress";
        let (dict, out_data) = make_stream(data, true);

        assert_eq!(
            dict.get_i64(b"Length"),
            Some(out_data.len() as i64)
        );
        assert_eq!(dict.get_name(b"Filter"), Some(b"FlateDecode".as_slice()));

        // Verify decompression gives back original
        let mut decoder = ZlibDecoder::new(&out_data[..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).unwrap();
        assert_eq!(decompressed, data);
    }
}
