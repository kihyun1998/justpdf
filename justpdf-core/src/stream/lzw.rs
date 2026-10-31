use crate::error::{JustPdfError, Result};
use crate::object::PdfDict;

const CLEAR: u32 = 256;
const EOD: u32 = 257;
const FIRST: usize = 258;
const MAX_ENTRIES: usize = 4096;
const MAX_WIDTH: u32 = 12;

/// One table entry: its string is the string of `prefix` followed by `byte`.
#[derive(Clone, Copy)]
struct Entry {
    prefix: u16,
    byte: u8,
    first: u8,
    len: u16,
}

/// Decode LZWDecode data.
///
/// Codes start 9 bits wide and widen up to 12; `/EarlyChange` 0 widens one
/// entry later than the default 1, and any other value reads as 1. The table
/// holds at most 4096 entries and a clear code empties it. Decoding ends at the
/// end-of-data code or when fewer bits than one code remain, returning what was
/// decoded.
pub fn decode(data: &[u8], params: Option<&PdfDict>) -> Result<Vec<u8>> {
    let early: usize = match params.and_then(|p| p.get_i64(b"EarlyChange")) {
        Some(0) => 0,
        _ => 1,
    };

    let mut table: Vec<Entry> = Vec::with_capacity(MAX_ENTRIES);
    table.extend((0..=255u8).map(|b| Entry {
        prefix: 0,
        byte: b,
        first: b,
        len: 1,
    }));
    // Placeholders for CLEAR and EOD, never read.
    table.extend([table[0]; 2]);

    let mut out = Vec::new();
    let total_bits = data.len() * 8;
    let mut bit_pos = 0usize;
    let mut width = 9u32;
    let mut prev: Option<u32> = None;

    while bit_pos + width as usize <= total_bits {
        let code = read_bits(data, bit_pos, width);
        bit_pos += width as usize;

        if code == CLEAR {
            table.truncate(FIRST);
            width = 9;
            prev = None;
            continue;
        }
        if code == EOD {
            break;
        }

        let next = table.len();
        let code_idx = code as usize;
        match prev {
            None if code_idx < 256 => {}
            Some(p) if code_idx < next => {
                if next < MAX_ENTRIES {
                    let first = table[code_idx].first;
                    table.push(extend(&table, p, first));
                }
            }
            Some(p) if code_idx == next => {
                let first = table[p as usize].first;
                table.push(extend(&table, p, first));
            }
            _ => {
                return Err(JustPdfError::StreamDecode {
                    filter: "LZWDecode".into(),
                    detail: format!("invalid code {code}, table size {next}"),
                });
            }
        }
        write_entry(&table, code_idx, &mut out);
        prev = Some(code);

        if table.len() + early >= 1 << width && width < MAX_WIDTH {
            width += 1;
        }
    }

    Ok(out)
}

/// The entry for the string of `prefix` followed by `byte`.
fn extend(table: &[Entry], prefix: u32, byte: u8) -> Entry {
    let p = table[prefix as usize];
    Entry {
        prefix: prefix as u16,
        byte,
        first: p.first,
        len: p.len + 1,
    }
}

/// Appends the string of entry `code` to `out`.
fn write_entry(table: &[Entry], code: usize, out: &mut Vec<u8>) {
    let len = table[code].len as usize;
    let start = out.len();
    out.resize(start + len, 0);
    let mut c = code;
    for slot in out[start..].iter_mut().rev() {
        let e = table[c];
        *slot = e.byte;
        c = e.prefix as usize;
    }
}

/// Reads `count` bits MSB first from `bit_offset`; the caller keeps them
/// inside `data`.
fn read_bits(data: &[u8], bit_offset: usize, count: u32) -> u32 {
    let mut val = 0u32;
    for i in 0..count as usize {
        let bit = bit_offset + i;
        val = (val << 1) | ((data[bit / 8] >> (7 - bit % 8)) & 1) as u32;
    }
    val
}
