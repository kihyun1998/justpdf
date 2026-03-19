mod ascii85;
mod ascii_hex;
pub mod ccitt;
pub mod dct;
mod flate;
mod predictor;
mod run_length;

use crate::error::{JustPdfError, Result};
use crate::object::{PdfDict, PdfObject};

/// Tolerant variant of [`decode_stream`] that attempts to recover partial data
/// from broken or truncated streams instead of returning an error.
///
/// Recovery strategies (tried in order):
/// 1. Normal decode (delegates to [`decode_stream`]).
/// 2. For single-filter streams: filter-specific partial decode (e.g. FlateDecode
///    partial decompression).
/// 3. For filter chains: try each filter individually, skipping any that fail.
pub fn decode_stream_tolerant(data: &[u8], dict: &PdfDict) -> Result<Vec<u8>> {
    // Strategy 1: try normal decode
    if let Ok(result) = decode_stream(data, dict) {
        return Ok(result);
    }

    let filters = get_filters(dict);
    let params_list = get_decode_params(dict);

    if filters.is_empty() {
        return Ok(data.to_vec());
    }

    // Strategy 2 & 3: walk the filter chain, recovering where possible
    let mut result = data.to_vec();

    for (i, filter) in filters.iter().enumerate() {
        let params = params_list.get(i).and_then(|p| p.as_ref());
        match decode_single_tolerant(&result, filter, params) {
            Ok(decoded) => result = decoded,
            Err(_) => {
                // Skip this filter entirely — keep the data as-is and continue
                // with the next filter in the chain.
            }
        }
    }

    Ok(result)
}

/// Tolerant single-filter decode: tries normal decode, then partial strategies.
fn decode_single_tolerant(data: &[u8], filter: &[u8], params: Option<&PdfDict>) -> Result<Vec<u8>> {
    // Try normal first
    if let Ok(result) = decode_single(data, filter, params) {
        return Ok(result);
    }

    // Filter-specific partial recovery
    match filter {
        b"FlateDecode" | b"Fl" => {
            let decoded = flate::decode_partial(data)?;
            if let Some(p) = params {
                // Try predictor; if it fails, return raw partial data
                predictor::apply(decoded.clone(), p).or(Ok(decoded))
            } else {
                Ok(decoded)
            }
        }
        // For unknown / unsupported filters, just pass through
        _ => Err(JustPdfError::StreamDecode {
            filter: String::from_utf8_lossy(filter).into_owned(),
            detail: "unsupported filter (tolerant)".into(),
        }),
    }
}

/// Decode a stream's raw data using the filters specified in its dictionary.
pub fn decode_stream(data: &[u8], dict: &PdfDict) -> Result<Vec<u8>> {
    let filters = get_filters(dict);
    let params_list = get_decode_params(dict);

    if filters.is_empty() {
        return Ok(data.to_vec());
    }

    let mut result = data.to_vec();

    for (i, filter) in filters.iter().enumerate() {
        let params = params_list.get(i).and_then(|p| p.as_ref());
        result = decode_single(&result, filter, params)?;
    }

    Ok(result)
}

/// Decode a single filter.
fn decode_single(data: &[u8], filter: &[u8], params: Option<&PdfDict>) -> Result<Vec<u8>> {
    match filter {
        b"FlateDecode" | b"Fl" => {
            let decoded = flate::decode(data)?;
            if let Some(p) = params {
                predictor::apply(decoded, p)
            } else {
                Ok(decoded)
            }
        }
        b"ASCIIHexDecode" | b"AHx" => ascii_hex::decode(data),
        b"ASCII85Decode" | b"A85" => ascii85::decode(data),
        b"LZWDecode" | b"LZW" => {
            let decoded = lzw_decode(data)?;
            if let Some(p) = params {
                predictor::apply(decoded, p)
            } else {
                Ok(decoded)
            }
        }
        b"RunLengthDecode" | b"RL" => run_length::decode(data),
        b"DCTDecode" | b"DCT" => {
            // JPEG decode → raw pixels; but for stream decoding we just
            // return the raw JPEG bytes (the image module handles actual decoding)
            Ok(data.to_vec())
        }
        b"JPXDecode" => {
            // JPEG2000: not yet implemented, pass through raw bytes
            Ok(data.to_vec())
        }
        b"JBIG2Decode" => {
            // JBIG2: not yet implemented, pass through raw bytes
            Ok(data.to_vec())
        }
        b"CCITTFaxDecode" | b"CCF" => {
            ccitt::decode(data, params)
        }
        b"Crypt" => {
            // Crypt filter is handled at the document level (transparent decryption).
            // By the time we reach here, the data has already been decrypted.
            Ok(data.to_vec())
        }
        _ => Err(JustPdfError::StreamDecode {
            filter: String::from_utf8_lossy(filter).into_owned(),
            detail: "unsupported filter".into(),
        }),
    }
}

/// Extract filter names from stream dict.
fn get_filters(dict: &PdfDict) -> Vec<Vec<u8>> {
    match dict.get(b"Filter") {
        Some(PdfObject::Name(name)) => vec![name.clone()],
        Some(PdfObject::Array(arr)) => arr
            .iter()
            .filter_map(|obj| match obj {
                PdfObject::Name(n) => Some(n.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Extract DecodeParms from stream dict.
fn get_decode_params(dict: &PdfDict) -> Vec<Option<PdfDict>> {
    match dict.get(b"DecodeParms") {
        Some(PdfObject::Dict(d)) => vec![Some(d.clone())],
        Some(PdfObject::Array(arr)) => arr
            .iter()
            .map(|obj| match obj {
                PdfObject::Dict(d) => Some(d.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Basic LZW decoder (PDF variant: early change = 1).
fn lzw_decode(data: &[u8]) -> Result<Vec<u8>> {
    // Minimal LZW implementation for PDF
    let mut result = Vec::new();
    let mut table: Vec<Vec<u8>> = (0..256).map(|i| vec![i as u8]).collect();
    // 256 = clear table, 257 = EOD
    table.push(Vec::new()); // 256
    table.push(Vec::new()); // 257

    let mut bit_pos: usize = 0;
    let mut code_size: u32 = 9;
    let mut prev_entry: Option<Vec<u8>> = None;

    loop {
        let code = read_bits(data, bit_pos, code_size);
        bit_pos += code_size as usize;

        if code == 256 {
            // Clear table
            table.truncate(258);
            code_size = 9;
            prev_entry = None;
            continue;
        }

        if code == 257 {
            // End of data
            break;
        }

        let entry = if (code as usize) < table.len() {
            table[code as usize].clone()
        } else if code as usize == table.len() {
            // Special case: code not yet in table
            let mut e = prev_entry.clone().unwrap_or_default();
            if let Some(first) = e.first().copied() {
                e.push(first);
            }
            e
        } else {
            return Err(JustPdfError::StreamDecode {
                filter: "LZWDecode".into(),
                detail: format!("invalid code {code}, table size {}", table.len()),
            });
        };

        result.extend_from_slice(&entry);

        if let Some(prev) = &prev_entry {
            let mut new_entry = prev.clone();
            if let Some(&first) = entry.first() {
                new_entry.push(first);
            }
            table.push(new_entry);
        }

        prev_entry = Some(entry);

        // Increase code size (early change: PDF default)
        if table.len() >= (1 << code_size) as usize && code_size < 12 {
            code_size += 1;
        }
    }

    Ok(result)
}

/// Read `count` bits from a big-endian bit stream.
fn read_bits(data: &[u8], bit_offset: usize, count: u32) -> u32 {
    let mut val: u32 = 0;
    for i in 0..count {
        let byte_idx = (bit_offset + i as usize) / 8;
        let bit_idx = 7 - ((bit_offset + i as usize) % 8);
        if byte_idx < data.len() {
            val = (val << 1) | ((data[byte_idx] >> bit_idx) & 1) as u32;
        } else {
            val <<= 1;
        }
    }
    val
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_filter() {
        let dict = PdfDict::new();
        let data = b"hello";
        let result = decode_stream(data, &dict).unwrap();
        assert_eq!(result, b"hello");
    }

    #[test]
    fn test_unsupported_filter() {
        let mut dict = PdfDict::new();
        dict.insert(
            b"Filter".to_vec(),
            PdfObject::Name(b"UnknownFilter".to_vec()),
        );
        let result = decode_stream(b"data", &dict);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_filters_single() {
        let mut dict = PdfDict::new();
        dict.insert(b"Filter".to_vec(), PdfObject::Name(b"FlateDecode".to_vec()));
        let filters = get_filters(&dict);
        assert_eq!(filters, vec![b"FlateDecode".to_vec()]);
    }

    #[test]
    fn test_get_filters_array() {
        let mut dict = PdfDict::new();
        dict.insert(
            b"Filter".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Name(b"ASCII85Decode".to_vec()),
                PdfObject::Name(b"FlateDecode".to_vec()),
            ]),
        );
        let filters = get_filters(&dict);
        assert_eq!(filters.len(), 2);
    }

    #[test]
    fn test_filter_chain_flate_then_ascii_hex() {
        use flate2::Compression;
        use flate2::write::ZlibEncoder;
        use std::io::Write;

        let original = b"Hello, filter chain!";

        // Step 1: FlateDecode compress
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(original).unwrap();
        let compressed = encoder.finish().unwrap();

        // Step 2: ASCIIHex encode the compressed bytes
        let mut hex_encoded: Vec<u8> = Vec::new();
        for &b in &compressed {
            hex_encoded.extend_from_slice(format!("{b:02X}").as_bytes());
        }
        hex_encoded.push(b'>');

        // Dict: Filter [ASCIIHexDecode FlateDecode] — applied in order
        let mut dict = PdfDict::new();
        dict.insert(
            b"Filter".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Name(b"ASCIIHexDecode".to_vec()),
                PdfObject::Name(b"FlateDecode".to_vec()),
            ]),
        );

        let result = decode_stream(&hex_encoded, &dict).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn test_filter_chain_ascii85_then_flate() {
        use flate2::Compression;
        use flate2::write::ZlibEncoder;
        use std::io::Write;

        let original = b"ASCII85 + Flate chain test data 1234567890";

        // Step 1: FlateDecode compress
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(original).unwrap();
        let compressed = encoder.finish().unwrap();

        // Step 2: ASCII85 encode
        let ascii85_encoded = ascii85_encode(&compressed);

        // Dict: Filter [ASCII85Decode FlateDecode]
        let mut dict = PdfDict::new();
        dict.insert(
            b"Filter".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Name(b"ASCII85Decode".to_vec()),
                PdfObject::Name(b"FlateDecode".to_vec()),
            ]),
        );

        let result = decode_stream(&ascii85_encoded, &dict).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn test_tolerant_normal_decode() {
        use flate2::Compression;
        use flate2::write::ZlibEncoder;
        use std::io::Write;

        let original = b"Tolerant decode should handle normal streams.";
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(original).unwrap();
        let compressed = encoder.finish().unwrap();

        let mut dict = PdfDict::new();
        dict.insert(b"Filter".to_vec(), PdfObject::Name(b"FlateDecode".to_vec()));

        let result = decode_stream_tolerant(&compressed, &dict).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn test_tolerant_truncated_flate() {
        use flate2::Compression;
        use flate2::write::ZlibEncoder;
        use std::io::Write;

        // Compress a large amount of data with low compression so there is
        // enough valid compressed content that partial decoding yields output.
        let original: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(&original).unwrap();
        let mut compressed = encoder.finish().unwrap();

        // Corrupt bytes in the second half of the compressed stream so that
        // the zlib decoder fails partway through (not at the very end where
        // trailing junk might be ignored).
        let corrupt_start = compressed.len() / 2;
        for b in &mut compressed[corrupt_start..corrupt_start + 64] {
            *b ^= 0xFF;
        }

        let mut dict = PdfDict::new();
        dict.insert(b"Filter".to_vec(), PdfObject::Name(b"FlateDecode".to_vec()));

        // Normal decode should fail on the corrupted data
        assert!(decode_stream(&compressed, &dict).is_err());

        // Tolerant decode should return some data rather than failing.
        // Depending on how the corruption affects the decompressor, we may get
        // partial or even full data (the decompressor may flush before hitting
        // the corrupted block).
        let result = decode_stream_tolerant(&compressed, &dict).unwrap();
        assert!(!result.is_empty(), "should recover some data");
        // The very beginning of the recovered data should match the original.
        let check_len = 256.min(result.len()).min(original.len());
        assert_eq!(&result[..check_len], &original[..check_len]);
    }

    #[test]
    fn test_tolerant_corrupted_middle() {
        use flate2::Compression;
        use flate2::write::ZlibEncoder;
        use std::io::Write;

        let original: Vec<u8> = (0..10_000).map(|i| (i % 256) as u8).collect();
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&original).unwrap();
        let mut compressed = encoder.finish().unwrap();

        // Corrupt some bytes in the middle of the compressed stream
        let mid = compressed.len() / 2;
        for b in &mut compressed[mid..mid + 10] {
            *b = 0xFF;
        }

        let mut dict = PdfDict::new();
        dict.insert(b"Filter".to_vec(), PdfObject::Name(b"FlateDecode".to_vec()));

        // Normal decode should fail
        assert!(decode_stream(&compressed, &dict).is_err());

        // Tolerant should recover the prefix before corruption
        let result = decode_stream_tolerant(&compressed, &dict).unwrap();
        assert!(!result.is_empty(), "should recover data before corruption");
    }

    #[test]
    fn test_tolerant_unknown_filter_in_chain_skipped() {
        use flate2::Compression;
        use flate2::write::ZlibEncoder;
        use std::io::Write;

        let original = b"Data through a chain with an unknown filter.";
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(original).unwrap();
        let compressed = encoder.finish().unwrap();

        // Chain: BogusFilter -> FlateDecode
        // Normal decode fails because BogusFilter is unknown.
        // Tolerant should skip BogusFilter and still apply FlateDecode.
        let mut dict = PdfDict::new();
        dict.insert(
            b"Filter".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Name(b"BogusFilter".to_vec()),
                PdfObject::Name(b"FlateDecode".to_vec()),
            ]),
        );

        // Normal decode should fail
        assert!(decode_stream(&compressed, &dict).is_err());

        // Tolerant should skip BogusFilter, apply FlateDecode, and recover original
        let result = decode_stream_tolerant(&compressed, &dict).unwrap();
        assert_eq!(result, original);
    }

    /// Simple ASCII85 encoder for testing.
    fn ascii85_encode(data: &[u8]) -> Vec<u8> {
        let mut result = Vec::new();
        for chunk in data.chunks(4) {
            if chunk.len() == 4 {
                let val = (chunk[0] as u32) << 24
                    | (chunk[1] as u32) << 16
                    | (chunk[2] as u32) << 8
                    | chunk[3] as u32;
                if val == 0 {
                    result.push(b'z');
                } else {
                    let mut digits = [0u8; 5];
                    let mut v = val;
                    for d in digits.iter_mut().rev() {
                        *d = (v % 85) as u8 + b'!';
                        v /= 85;
                    }
                    result.extend_from_slice(&digits);
                }
            } else {
                // Partial last group
                let mut val = 0u32;
                for (i, &b) in chunk.iter().enumerate() {
                    val |= (b as u32) << (24 - i * 8);
                }
                let mut digits = [0u8; 5];
                let mut v = val;
                for d in digits.iter_mut().rev() {
                    *d = (v % 85) as u8 + b'!';
                    v /= 85;
                }
                result.extend_from_slice(&digits[..chunk.len() + 1]);
            }
        }
        result.extend_from_slice(b"~>");
        result
    }
}
