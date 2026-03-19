mod ascii85;
mod ascii_hex;
pub mod ccitt;
pub mod dct;
mod flate;
mod predictor;
mod run_length;

use crate::error::{JustPdfError, Result};
use crate::object::{PdfDict, PdfObject};

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
