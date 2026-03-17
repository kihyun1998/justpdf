use std::collections::HashMap;

use crate::error::{JustPdfError, Result};
use crate::object::{self, PdfDict, PdfObject};
use crate::tokenizer::Tokenizer;
use crate::tokenizer::reader::PdfReader;

/// A single entry in the cross-reference table.
#[derive(Debug, Clone)]
pub enum XrefEntry {
    /// Free object.
    Free { next_free: u32, gen_num: u16 },
    /// In-use object at a byte offset.
    InUse { offset: u64, gen_num: u16 },
    /// Compressed object inside an object stream (PDF 1.5+).
    Compressed {
        obj_stream_num: u32,
        index_within: u16,
    },
}

/// The complete cross-reference table with merged trailer.
#[derive(Debug)]
pub struct Xref {
    pub entries: HashMap<u32, XrefEntry>,
    pub trailer: PdfDict,
}

impl Default for Xref {
    fn default() -> Self {
        Self::new()
    }
}

impl Xref {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            trailer: PdfDict::new(),
        }
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get entry for an object number.
    pub fn get(&self, obj_num: u32) -> Option<&XrefEntry> {
        self.entries.get(&obj_num)
    }

    /// Get /Size from trailer.
    pub fn size(&self) -> u32 {
        self.trailer.get_i64(b"Size").unwrap_or(0) as u32
    }
}

/// Parse a traditional xref table at the given offset.
/// Returns the entries and the trailer dictionary.
pub fn parse_xref_table(data: &[u8], offset: usize) -> Result<(Vec<(u32, XrefEntry)>, PdfDict)> {
    let mut reader = PdfReader::new_at(data, offset);

    // Expect "xref"
    let remaining = reader.remaining();
    if !remaining.starts_with(b"xref") {
        return Err(JustPdfError::InvalidXref {
            offset,
            detail: "expected 'xref' keyword".into(),
        });
    }
    reader.advance(4);
    reader.skip_whitespace();

    let mut entries = Vec::new();

    // Parse subsections: each starts with "start_obj count"
    loop {
        // Check if we hit "trailer"
        let remaining = reader.remaining();
        if remaining.starts_with(b"trailer") {
            break;
        }
        if reader.is_eof() {
            return Err(JustPdfError::TrailerNotFound);
        }

        // Read start object number and count
        let start_obj = read_ascii_number(&mut reader)?;
        reader.skip_whitespace();
        let count = read_ascii_number(&mut reader)?;
        reader.skip_whitespace();

        // Each entry: "nnnnnnnnnn ggggg n \r\n" (20 bytes typical)
        // Format: 10-digit offset, space, 5-digit gen, space, 'n'/'f', EOL
        // Be tolerant of different line endings and spacing.
        for i in 0..count {
            let entry_start = reader.pos();

            // Read offset (10 digits)
            let mut offset_buf = Vec::new();
            while let Some(b) = reader.peek() {
                if b.is_ascii_digit() {
                    offset_buf.push(b);
                    reader.advance(1);
                } else {
                    break;
                }
            }
            reader.skip_whitespace();

            // Read generation (5 digits)
            let mut gen_buf = Vec::new();
            while let Some(b) = reader.peek() {
                if b.is_ascii_digit() {
                    gen_buf.push(b);
                    reader.advance(1);
                } else {
                    break;
                }
            }
            reader.skip_whitespace();

            // Read type char: 'n' or 'f'
            let type_char = reader.next_byte().unwrap_or(b' ');
            // Skip trailing whitespace/EOL
            reader.skip_whitespace();

            let offset_str = std::str::from_utf8(&offset_buf).unwrap_or("0");
            let gen_str = std::str::from_utf8(&gen_buf).unwrap_or("0");
            let offset_val: u64 = offset_str.parse().unwrap_or(0);
            let gen_val: u16 = gen_str.parse().unwrap_or(0);

            let obj_num = start_obj + i;

            let entry = match type_char {
                b'n' => XrefEntry::InUse {
                    offset: offset_val,
                    gen_num: gen_val,
                },
                b'f' => XrefEntry::Free {
                    next_free: offset_val as u32,
                    gen_num: gen_val,
                },
                _ => {
                    return Err(JustPdfError::InvalidXref {
                        offset: entry_start,
                        detail: format!("unknown xref entry type: {:?}", type_char as char),
                    });
                }
            };

            entries.push((obj_num, entry));
        }
    }

    // Parse trailer dictionary
    reader.advance(7); // skip "trailer"
    reader.skip_whitespace();

    let mut tokenizer = Tokenizer::new_at(data, reader.pos());
    let trailer_obj = object::parse_object(&mut tokenizer)?;

    let trailer = match trailer_obj {
        PdfObject::Dict(d) => d,
        _ => {
            return Err(JustPdfError::TrailerNotFound);
        }
    };

    Ok((entries, trailer))
}

/// Read a decimal number from current position.
fn read_ascii_number(reader: &mut PdfReader<'_>) -> Result<u32> {
    let start = reader.pos();
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
        return Err(JustPdfError::InvalidXref {
            offset: start,
            detail: "expected number".into(),
        });
    }
    let s = std::str::from_utf8(&digits).unwrap();
    s.parse::<u32>().map_err(|_| JustPdfError::InvalidXref {
        offset: start,
        detail: format!("invalid number: {s}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_xref_table() {
        let xref_data = b"xref\n\
            0 3\n\
            0000000000 65535 f \r\n\
            0000000100 00000 n \r\n\
            0000000200 00000 n \r\n\
            trailer\n\
            << /Size 3 /Root 1 0 R >>";

        let (entries, trailer) = parse_xref_table(xref_data, 0).unwrap();

        assert_eq!(entries.len(), 3);

        // Entry 0: free
        match &entries[0] {
            (
                0,
                XrefEntry::Free {
                    next_free: 0,
                    gen_num: 65535,
                },
            ) => {}
            other => panic!("unexpected entry 0: {other:?}"),
        }

        // Entry 1: in use at offset 100
        match &entries[1] {
            (
                1,
                XrefEntry::InUse {
                    offset: 100,
                    gen_num: 0,
                },
            ) => {}
            other => panic!("unexpected entry 1: {other:?}"),
        }

        // Entry 2: in use at offset 200
        match &entries[2] {
            (
                2,
                XrefEntry::InUse {
                    offset: 200,
                    gen_num: 0,
                },
            ) => {}
            other => panic!("unexpected entry 2: {other:?}"),
        }

        assert_eq!(trailer.get_i64(b"Size"), Some(3));
    }

    #[test]
    fn test_parse_xref_table_multiple_subsections() {
        let xref_data = b"xref\n\
            0 1\n\
            0000000000 65535 f \r\n\
            3 2\n\
            0000000300 00000 n \r\n\
            0000000400 00000 n \r\n\
            trailer\n\
            << /Size 5 >>";

        let (entries, _trailer) = parse_xref_table(xref_data, 0).unwrap();
        assert_eq!(entries.len(), 3);

        let obj_nums: Vec<u32> = entries.iter().map(|(n, _)| *n).collect();
        assert_eq!(obj_nums, vec![0, 3, 4]);
    }
}
