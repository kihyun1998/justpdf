mod table;

pub use table::{Xref, XrefEntry};

use crate::error::{JustPdfError, Result};
use crate::object::{PdfDict, PdfObject};
use crate::stream;
use crate::tokenizer::Tokenizer;
use crate::tokenizer::reader::PdfReader;

/// Find the startxref offset by scanning backward from EOF.
pub fn find_startxref(data: &[u8]) -> Result<usize> {
    // Search backward for "startxref"
    let needle = b"startxref";
    let search_len = data.len().min(1024);
    let search_start = data.len() - search_len;

    for i in (search_start..data.len().saturating_sub(needle.len())).rev() {
        if &data[i..i + needle.len()] == needle {
            // Read the offset value after "startxref"
            let mut reader = PdfReader::new_at(data, i + needle.len());
            reader.skip_whitespace();
            let mut digits = Vec::new();
            while let Some(b) = reader.peek() {
                if b.is_ascii_digit() {
                    digits.push(b);
                    reader.advance(1);
                } else {
                    break;
                }
            }
            if digits.is_empty() {
                return Err(JustPdfError::StartXrefNotFound);
            }
            let s = std::str::from_utf8(&digits).unwrap();
            return s
                .parse::<usize>()
                .map_err(|_| JustPdfError::StartXrefNotFound);
        }
    }

    Err(JustPdfError::StartXrefNotFound)
}

/// Load the complete xref (following /Prev chain) and merged trailer.
pub fn load_xref(data: &[u8]) -> Result<Xref> {
    let startxref = find_startxref(data)?;
    load_xref_at(data, startxref)
}

/// Load xref starting from a specific offset, following /Prev chain.
fn load_xref_at(data: &[u8], offset: usize) -> Result<Xref> {
    let mut merged = Xref::new();
    let mut current_offset = Some(offset);
    let mut visited = std::collections::HashSet::new();

    while let Some(off) = current_offset {
        if !visited.insert(off) {
            // Circular /Prev chain
            break;
        }

        if off >= data.len() {
            return Err(JustPdfError::InvalidXref {
                offset: off,
                detail: "xref offset beyond file size".into(),
            });
        }

        // Determine if this is a traditional xref table or an xref stream
        let reader = PdfReader::new_at(data, off);
        let remaining = reader.remaining();

        if remaining.starts_with(b"xref") {
            // Traditional xref table
            let (entries, trailer) = table::parse_xref_table(data, off)?;
            // Entries from earlier xref sections don't override later ones
            for (obj_num, entry) in entries {
                merged.entries.entry(obj_num).or_insert(entry);
            }
            // First trailer wins for main keys, /Prev is used for chaining
            let prev = trailer.get_i64(b"Prev").map(|v| v as usize);
            if merged.trailer.is_empty() {
                merged.trailer = trailer;
            }
            current_offset = prev;
        } else {
            // Xref stream
            let (entries, trailer) = parse_xref_stream(data, off)?;
            for (obj_num, entry) in entries {
                merged.entries.entry(obj_num).or_insert(entry);
            }
            let prev = trailer.get_i64(b"Prev").map(|v| v as usize);
            if merged.trailer.is_empty() {
                merged.trailer = trailer;
            }
            current_offset = prev;
        }
    }

    if merged.trailer.is_empty() {
        return Err(JustPdfError::TrailerNotFound);
    }

    Ok(merged)
}

/// Parse an xref stream object at the given offset.
fn parse_xref_stream(data: &[u8], offset: usize) -> Result<(Vec<(u32, XrefEntry)>, PdfDict)> {
    use crate::object;

    let mut tokenizer = Tokenizer::new_at(data, offset);
    let (_iref, obj) = object::parse_indirect_object(&mut tokenizer)?;

    let (dict, raw_data) = match obj {
        PdfObject::Stream { dict, data } => (dict, data),
        _ => {
            return Err(JustPdfError::InvalidXref {
                offset,
                detail: "expected xref stream object".into(),
            });
        }
    };

    // Decode the stream
    let decoded = stream::decode_stream(&raw_data, &dict)?;

    // Parse W array
    let w = dict
        .get_array(b"W")
        .ok_or_else(|| JustPdfError::InvalidXref {
            offset,
            detail: "missing /W in xref stream".into(),
        })?;

    if w.len() != 3 {
        return Err(JustPdfError::InvalidXref {
            offset,
            detail: format!("/W array must have 3 elements, got {}", w.len()),
        });
    }

    let w0 = w[0].as_i64().unwrap_or(0) as usize;
    let w1 = w[1].as_i64().unwrap_or(0) as usize;
    let w2 = w[2].as_i64().unwrap_or(0) as usize;
    let entry_size = w0 + w1 + w2;

    if entry_size == 0 {
        return Ok((Vec::new(), dict));
    }

    // Parse Index array (or default)
    let index_pairs = if let Some(idx) = dict.get_array(b"Index") {
        let mut pairs = Vec::new();
        for chunk in idx.chunks(2) {
            if chunk.len() == 2 {
                let start = chunk[0].as_i64().unwrap_or(0) as u32;
                let count = chunk[1].as_i64().unwrap_or(0) as u32;
                pairs.push((start, count));
            }
        }
        pairs
    } else {
        let size = dict.get_i64(b"Size").unwrap_or(0) as u32;
        vec![(0, size)]
    };

    let mut entries = Vec::new();
    let mut data_pos = 0;

    for (start, count) in index_pairs {
        for i in 0..count {
            if data_pos + entry_size > decoded.len() {
                break;
            }

            let field0 = read_field(&decoded[data_pos..], w0);
            let field1 = read_field(&decoded[data_pos + w0..], w1);
            let field2 = read_field(&decoded[data_pos + w0 + w1..], w2);
            data_pos += entry_size;

            let obj_num = start + i;
            let typ = if w0 == 0 { 1 } else { field0 }; // default type is 1

            let entry = match typ {
                0 => XrefEntry::Free {
                    next_free: field1 as u32,
                    gen_num: field2 as u16,
                },
                1 => XrefEntry::InUse {
                    offset: field1 as u64,
                    gen_num: field2 as u16,
                },
                2 => XrefEntry::Compressed {
                    obj_stream_num: field1 as u32,
                    index_within: field2 as u16,
                },
                _ => continue, // Unknown type, skip
            };

            entries.push((obj_num, entry));
        }
    }

    Ok((entries, dict))
}

/// Read a big-endian integer field of `width` bytes from data.
fn read_field(data: &[u8], width: usize) -> u64 {
    let mut val: u64 = 0;
    for i in 0..width {
        if i < data.len() {
            val = (val << 8) | data[i] as u64;
        }
    }
    val
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_startxref() {
        let pdf = b"%PDF-1.4\nsome content\nstartxref\n1234\n%%EOF";
        let offset = find_startxref(pdf).unwrap();
        assert_eq!(offset, 1234);
    }

    #[test]
    fn test_find_startxref_missing() {
        let data = b"this is not a pdf";
        assert!(find_startxref(data).is_err());
    }

    #[test]
    fn test_read_field() {
        assert_eq!(read_field(&[0x00, 0x01], 2), 1);
        assert_eq!(read_field(&[0x01, 0x00], 2), 256);
        assert_eq!(read_field(&[], 0), 0);
        assert_eq!(read_field(&[0xFF], 1), 255);
    }
}
