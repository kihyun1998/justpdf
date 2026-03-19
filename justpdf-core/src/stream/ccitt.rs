//! CCITT Fax (Group 3 / Group 4) decoder for PDF streams.
//!
//! Implements ITU-T T.4 (Group 3) and T.6 (Group 4) decompression
//! as specified in the PDF Reference for the CCITTFaxDecode filter.

use crate::error::{JustPdfError, Result};
use crate::object::PdfDict;

/// Parameters extracted from the CCITTFaxDecode DecodeParms dictionary.
#[derive(Debug, Clone)]
pub struct CcittParams {
    /// K parameter: <0 = Group 4, 0 = Group 3 (1D only), >0 = Group 3 (mixed 1D/2D)
    pub k: i64,
    /// Image width in pixels (columns).
    pub columns: u32,
    /// Image height in pixels (rows). 0 = unknown.
    pub rows: u32,
    /// Whether EOL markers are expected.
    pub end_of_line: bool,
    /// Whether each encoded line is byte-aligned.
    pub encoded_byte_align: bool,
    /// Whether end-of-block pattern is present.
    pub end_of_block: bool,
    /// If true, 1-bits are black; otherwise 0-bits are black (default: false → 0=black).
    pub black_is1: bool,
}

impl CcittParams {
    /// Parse CCITT parameters from a DecodeParms dictionary.
    pub fn from_dict(params: Option<&PdfDict>) -> Self {
        let params = match params {
            Some(p) => p,
            None => {
                return Self {
                    k: 0,
                    columns: 1728,
                    rows: 0,
                    end_of_line: false,
                    encoded_byte_align: false,
                    end_of_block: true,
                    black_is1: false,
                };
            }
        };

        Self {
            k: params.get_i64(b"K").unwrap_or(0),
            columns: params.get_i64(b"Columns").unwrap_or(1728) as u32,
            rows: params.get_i64(b"Rows").unwrap_or(0) as u32,
            end_of_line: params
                .get(b"EndOfLine")
                .and_then(|o| o.as_bool())
                .unwrap_or(false),
            encoded_byte_align: params
                .get(b"EncodedByteAlign")
                .and_then(|o| o.as_bool())
                .unwrap_or(false),
            end_of_block: params
                .get(b"EndOfBlock")
                .and_then(|o| o.as_bool())
                .unwrap_or(true),
            black_is1: params
                .get(b"BlackIs1")
                .and_then(|o| o.as_bool())
                .unwrap_or(false),
        }
    }
}

/// Decode CCITT-encoded data. Returns 1-byte-per-pixel data (0x00=white, 0xFF=black),
/// with dimensions `columns x rows`.
pub fn decode(data: &[u8], params: Option<&PdfDict>) -> Result<Vec<u8>> {
    let p = CcittParams::from_dict(params);
    let columns = p.columns as usize;
    if columns == 0 {
        return Ok(Vec::new());
    }

    let mut reader = BitReader::new(data);
    let mut output_rows: Vec<Vec<u8>> = Vec::new();

    if p.k < 0 {
        // Pure Group 4 (2D)
        decode_group4(&mut reader, &p, columns, &mut output_rows)?;
    } else if p.k == 0 {
        // Pure Group 3 (1D only)
        decode_group3_1d(&mut reader, &p, columns, &mut output_rows)?;
    } else {
        // Mixed Group 3 (1D/2D)
        decode_group3_mixed(&mut reader, &p, columns, &mut output_rows)?;
    }

    // Convert rows to output: 1 byte per pixel
    let height = if p.rows > 0 {
        p.rows as usize
    } else {
        output_rows.len()
    };

    let mut result = Vec::with_capacity(columns * height);
    for i in 0..height {
        if i < output_rows.len() {
            let row = &output_rows[i];
            for j in 0..columns {
                let pixel = if j < row.len() { row[j] } else { 0 };
                // Convert: in the row, 1=black, 0=white
                // Output: BlackIs1 controls interpretation
                if p.black_is1 {
                    result.push(if pixel != 0 { 0xFF } else { 0x00 });
                } else {
                    result.push(if pixel != 0 { 0x00 } else { 0xFF });
                }
            }
        } else {
            // Pad with white
            result.resize(result.len() + columns, 0xFF);
        }
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Group 3 1D (Modified Huffman)
// ---------------------------------------------------------------------------

fn decode_group3_1d(
    reader: &mut BitReader,
    params: &CcittParams,
    columns: usize,
    output: &mut Vec<Vec<u8>>,
) -> Result<()> {
    let max_rows = if params.rows > 0 {
        params.rows as usize
    } else {
        100_000
    };

    for _ in 0..max_rows {
        // Skip EOL if present
        if params.end_of_line {
            skip_eol(reader);
        }
        if params.encoded_byte_align {
            reader.align_byte();
        }

        // Check for end-of-data
        if reader.is_eof() {
            break;
        }

        match decode_1d_line(reader, columns) {
            Ok(row) => output.push(row),
            Err(_) => break,
        }

        if params.rows == 0 && reader.is_eof() {
            break;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Group 3 Mixed (1D/2D)
// ---------------------------------------------------------------------------

fn decode_group3_mixed(
    reader: &mut BitReader,
    params: &CcittParams,
    columns: usize,
    output: &mut Vec<Vec<u8>>,
) -> Result<()> {
    let max_rows = if params.rows > 0 {
        params.rows as usize
    } else {
        100_000
    };

    // First line is always 1D
    // In mixed mode, after EOL, a tag bit indicates 1D (1) or 2D (0)
    let white_ref = vec![0u8; columns];

    for _ in 0..max_rows {
        if params.end_of_line {
            skip_eol(reader);
        }
        if params.encoded_byte_align {
            reader.align_byte();
        }

        if reader.is_eof() {
            break;
        }

        // Read tag bit for mixed mode
        let tag = reader.read_bit();
        if tag.is_none() {
            break;
        }

        let row = if tag == Some(1) {
            // 1D line
            match decode_1d_line(reader, columns) {
                Ok(r) => r,
                Err(_) => break,
            }
        } else {
            // 2D line
            let ref_line = output.last().unwrap_or(&white_ref);
            match decode_2d_line(reader, ref_line, columns) {
                Ok(r) => r,
                Err(_) => break,
            }
        };

        output.push(row);

        if params.rows == 0 && reader.is_eof() {
            break;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Group 4 (2D)
// ---------------------------------------------------------------------------

fn decode_group4(
    reader: &mut BitReader,
    params: &CcittParams,
    columns: usize,
    output: &mut Vec<Vec<u8>>,
) -> Result<()> {
    let max_rows = if params.rows > 0 {
        params.rows as usize
    } else {
        100_000
    };

    let white_ref = vec![0u8; columns];

    for _ in 0..max_rows {
        if params.encoded_byte_align {
            reader.align_byte();
        }

        if reader.is_eof() {
            break;
        }

        // Check for EOFB (two consecutive EOL codes = 000000000001 000000000001)
        if params.end_of_block && reader.peek_bits(24) == Some(0x001001) {
            break;
        }

        let ref_line = output.last().unwrap_or(&white_ref);
        match decode_2d_line(reader, ref_line, columns) {
            Ok(row) => output.push(row),
            Err(_) => break,
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// 1D line decoder (Modified Huffman)
// ---------------------------------------------------------------------------

fn decode_1d_line(reader: &mut BitReader, columns: usize) -> Result<Vec<u8>> {
    let mut row = vec![0u8; columns];
    let mut pos: usize = 0;
    let mut is_white = true; // Always start with white run

    while pos < columns {
        let run_len = if is_white {
            read_white_run(reader)?
        } else {
            read_black_run(reader)?
        };

        let end = (pos + run_len).min(columns);
        if !is_white {
            row[pos..end].fill(1); // black
        }
        pos = end;
        is_white = !is_white;
    }

    Ok(row)
}

// ---------------------------------------------------------------------------
// 2D line decoder (Group 3 2D / Group 4)
// ---------------------------------------------------------------------------

fn decode_2d_line(
    reader: &mut BitReader,
    ref_line: &[u8],
    columns: usize,
) -> Result<Vec<u8>> {
    let mut row = vec![0u8; columns];
    let mut a0: i64 = -1; // current position (-1 = imaginary white pixel before line start)
    let mut is_white = true; // color at a0 (starts white)

    while (a0 as usize) < columns {
        let mode = read_2d_mode(reader)?;
        match mode {
            Mode2D::Pass => {
                // b2 = second changing element on reference line to the right of a0
                let b1 = find_b1(ref_line, a0, is_white, columns);
                let b2 = find_next_change(ref_line, b1, columns);
                a0 = b2 as i64;
                // Color doesn't change after pass
            }
            Mode2D::Horizontal => {
                // Read two run-lengths: a0a1 and a1a2
                let run1 = if is_white {
                    read_white_run(reader)?
                } else {
                    read_black_run(reader)?
                };
                let run2 = if is_white {
                    read_black_run(reader)?
                } else {
                    read_white_run(reader)?
                };

                let start = if a0 < 0 { 0 } else { a0 as usize };

                // First run (a0a1)
                let end1 = (start + run1).min(columns);
                if !is_white {
                    row[start..end1].fill(1);
                }

                // Second run (a1a2)
                let end2 = (end1 + run2).min(columns);
                if is_white {
                    // a1a2 is opposite color = black
                    row[end1..end2].fill(1);
                }
                // else: a1a2 is white, row already 0

                a0 = end2 as i64;
                // Color returns to original after horizontal
            }
            Mode2D::Vertical(offset) => {
                let b1 = find_b1(ref_line, a0, is_white, columns) as i64;
                let a1 = (b1 + offset).max(0).min(columns as i64);

                let start = if a0 < 0 { 0 } else { a0 as usize };
                let end = a1 as usize;

                if !is_white {
                    row[start..end.min(columns)].fill(1);
                }

                a0 = a1;
                is_white = !is_white;
            }
        }
    }

    Ok(row)
}

/// Find b1: the first changing element on the reference line to the right of a0
/// whose color is opposite to the current coding color.
fn find_b1(ref_line: &[u8], a0: i64, is_white: bool, columns: usize) -> usize {
    let start = if a0 < 0 { 0 } else { (a0 + 1) as usize };
    let current_color: u8 = if is_white { 0 } else { 1 };

    // b1 must be a changing element with the opposite color
    // First, skip to where ref_line has the opposite color
    let mut pos = start;

    // We need to find a position where ref_line changes AND the new color is opposite to current
    // b1 definition: first changing element on ref line right of a0 with opposite color to a0's color
    while pos < columns {
        let ref_color = ref_line.get(pos).copied().unwrap_or(0);
        if ref_color != current_color {
            // Found opposite color; check it's a changing element (different from previous)
            let prev_color = if pos > 0 {
                ref_line.get(pos - 1).copied().unwrap_or(0)
            } else {
                0 // imaginary white
            };
            if ref_color != prev_color || pos == start {
                return pos;
            }
        }
        pos += 1;
    }

    columns
}

/// Find the next changing element after position `pos` on the reference line.
fn find_next_change(ref_line: &[u8], pos: usize, columns: usize) -> usize {
    if pos >= columns {
        return columns;
    }
    let current = ref_line.get(pos).copied().unwrap_or(0);
    let mut p = pos + 1;
    while p < columns {
        if ref_line[p] != current {
            return p;
        }
        p += 1;
    }
    columns
}

#[derive(Debug)]
enum Mode2D {
    Pass,
    Horizontal,
    Vertical(i64), // offset: -3..+3
}

fn read_2d_mode(reader: &mut BitReader) -> Result<Mode2D> {
    // 2D mode codes (ITU-T T.6 Table 7):
    // 1        -> V(0)
    // 011      -> VR(1)
    // 010      -> VL(1)
    // 0011     -> H
    // 0001     -> P (pass)
    // 000011   -> VR(2)
    // 000010   -> VL(2)
    // 0000011  -> VR(3)
    // 0000010  -> VL(3)

    let b = reader.read_bit().ok_or_else(|| JustPdfError::StreamDecode {
        filter: "CCITTFaxDecode".into(),
        detail: "unexpected end of data in 2D mode".into(),
    })?;

    if b == 1 {
        return Ok(Mode2D::Vertical(0));
    }

    let b = reader.read_bit().ok_or_else(eof_err)?;
    if b == 1 {
        let b = reader.read_bit().ok_or_else(eof_err)?;
        return if b == 1 {
            Ok(Mode2D::Vertical(1)) // VR(1)
        } else {
            Ok(Mode2D::Vertical(-1)) // VL(1)
        };
    }

    let b = reader.read_bit().ok_or_else(eof_err)?;
    if b == 1 {
        let b = reader.read_bit().ok_or_else(eof_err)?;
        return if b == 1 {
            Ok(Mode2D::Horizontal)
        } else {
            Ok(Mode2D::Pass)
        };
    }

    let b = reader.read_bit().ok_or_else(eof_err)?;
    if b == 1 {
        let b = reader.read_bit().ok_or_else(eof_err)?;
        return if b == 1 {
            Ok(Mode2D::Vertical(2))
        } else {
            Ok(Mode2D::Vertical(-2))
        };
    }

    let b = reader.read_bit().ok_or_else(eof_err)?;
    if b == 1 {
        let b = reader.read_bit().ok_or_else(eof_err)?;
        return if b == 1 {
            Ok(Mode2D::Vertical(3))
        } else {
            Ok(Mode2D::Vertical(-3))
        };
    }

    // Unknown code — try to recover
    Err(JustPdfError::StreamDecode {
        filter: "CCITTFaxDecode".into(),
        detail: "invalid 2D mode code".into(),
    })
}

fn eof_err() -> JustPdfError {
    JustPdfError::StreamDecode {
        filter: "CCITTFaxDecode".into(),
        detail: "unexpected end of data".into(),
    }
}

// ---------------------------------------------------------------------------
// Huffman run-length decoders (Modified Huffman codes from ITU-T T.4)
// ---------------------------------------------------------------------------

fn read_white_run(reader: &mut BitReader) -> Result<usize> {
    let mut total = 0usize;
    // Read makeup codes first, then terminating code
    loop {
        let code = read_white_code(reader)?;
        total += code;
        if code < 64 {
            // Terminating code
            break;
        }
    }
    Ok(total)
}

fn read_black_run(reader: &mut BitReader) -> Result<usize> {
    let mut total = 0usize;
    loop {
        let code = read_black_code(reader)?;
        total += code;
        if code < 64 {
            break;
        }
    }
    Ok(total)
}

/// Read a single white Huffman code (terminating or makeup).
fn read_white_code(reader: &mut BitReader) -> Result<usize> {
    // White codes from ITU-T T.4 Table 2 & 3
    let mut bits: u32 = 0;
    let mut len: u32 = 0;

    for _ in 0..25 {
        let b = reader.read_bit().ok_or_else(eof_err)?;
        bits = (bits << 1) | b as u32;
        len += 1;

        if let Some(run) = match_white_code(bits, len) {
            return Ok(run);
        }
    }

    // Failed to match — return 0 to avoid infinite loop
    Ok(0)
}

fn read_black_code(reader: &mut BitReader) -> Result<usize> {
    let mut bits: u32 = 0;
    let mut len: u32 = 0;

    for _ in 0..25 {
        let b = reader.read_bit().ok_or_else(eof_err)?;
        bits = (bits << 1) | b as u32;
        len += 1;

        if let Some(run) = match_black_code(bits, len) {
            return Ok(run);
        }
    }

    Ok(0)
}

/// Match white Huffman code (ITU-T T.4 Tables 2 & 3).
fn match_white_code(bits: u32, len: u32) -> Option<usize> {
    // White terminating codes (run length 0-63)
    match (len, bits) {
        (4, 0b0111) => Some(2),
        (4, 0b1000) => Some(3),
        (4, 0b1011) => Some(4),
        (4, 0b1100) => Some(5),
        (4, 0b1110) => Some(6),
        (4, 0b1111) => Some(7),
        (5, 0b10011) => Some(8),
        (5, 0b10100) => Some(9),
        (5, 0b00111) => Some(10),
        (5, 0b01000) => Some(11),
        (5, 0b11000) => Some(128), // makeup
        (6, 0b000111) => Some(1),
        (6, 0b001000) => Some(12),
        (6, 0b001011) => Some(14),
        (6, 0b000011) => Some(15),
        (6, 0b110100) => Some(16),
        (6, 0b110101) => Some(17),
        (6, 0b101010) => Some(192), // makeup
        (6, 0b101011) => Some(1664), // makeup
        (7, 0b0100111) => Some(13),
        (7, 0b0011000) => Some(18),
        (7, 0b0001000) => Some(19),
        (7, 0b0010111) => Some(20),
        (7, 0b0000011) => Some(21),
        (7, 0b0000100) => Some(22),
        (7, 0b0101000) => Some(23),
        (7, 0b0101011) => Some(24),
        (7, 0b0010011) => Some(25),
        (7, 0b0100100) => Some(26),
        // (7, 0b0011000) already matched above for run=18
        (7, 0b0000010) => Some(256), // makeup
        (8, 0b00110101) => Some(0),
        (8, 0b00000010) => Some(27),
        (8, 0b00000011) => Some(28),
        (8, 0b00011010) => Some(29),
        (8, 0b00011011) => Some(30),
        (8, 0b00010010) => Some(31),
        (8, 0b00010011) => Some(32),
        (8, 0b00010100) => Some(33),
        (8, 0b00010101) => Some(34),
        (8, 0b00010110) => Some(35),
        (8, 0b00010111) => Some(36),
        (8, 0b00101000) => Some(37),
        (8, 0b00101001) => Some(38),
        (8, 0b00101010) => Some(39),
        (8, 0b00101011) => Some(40),
        (8, 0b00101100) => Some(41),
        (8, 0b00101101) => Some(42),
        (8, 0b00000100) => Some(43),
        (8, 0b00000101) => Some(44),
        (8, 0b00001010) => Some(45),
        (8, 0b00001011) => Some(46),
        (8, 0b01010010) => Some(47),
        (8, 0b01010011) => Some(48),
        (8, 0b01010100) => Some(49),
        (8, 0b01010101) => Some(50),
        (8, 0b00100100) => Some(51),
        (8, 0b00100101) => Some(52),
        (8, 0b01011000) => Some(53),
        (8, 0b01011001) => Some(54),
        (8, 0b01011010) => Some(55),
        (8, 0b01011011) => Some(56),
        (8, 0b01001010) => Some(57),
        (8, 0b01001011) => Some(58),
        (8, 0b00110010) => Some(59),
        (8, 0b00110011) => Some(60),
        (8, 0b00110100) => Some(61),
        (8, 0b00110110) => Some(320), // makeup
        (8, 0b00110111) => Some(384), // makeup
        (8, 0b01100100) => Some(448), // makeup
        (8, 0b01100101) => Some(512), // makeup
        (8, 0b01101000) => Some(576), // makeup (added to fix)
        (8, 0b01100111) => Some(640), // makeup
        (9, 0b011001100) => Some(62),
        (9, 0b011001101) => Some(63),
        (9, 0b011010010) => Some(704), // makeup
        (9, 0b011010011) => Some(768), // makeup
        (9, 0b011010100) => Some(832), // makeup
        (9, 0b011010101) => Some(896), // makeup
        (9, 0b011010110) => Some(960), // makeup
        (9, 0b011010111) => Some(1024), // makeup
        (9, 0b011011000) => Some(1088), // makeup
        (9, 0b011011001) => Some(1152), // makeup
        (9, 0b011011010) => Some(1216), // makeup
        (9, 0b011011011) => Some(1280), // makeup
        (9, 0b011011100) => Some(1344), // makeup
        (9, 0b011011101) => Some(1408), // makeup
        (9, 0b011011110) => Some(1472), // makeup
        (9, 0b011011111) => Some(1536), // makeup
        (9, 0b011001000) => Some(1600), // makeup
        (11, 0b00000001000) => Some(1728), // makeup
        (12, 0b000000010010) => Some(1792), // makeup (shared w/ black)
        (12, 0b000000010011) => Some(1856),
        (12, 0b000000010100) => Some(1920),
        (12, 0b000000010101) => Some(1984),
        (12, 0b000000010110) => Some(2048),
        (12, 0b000000010111) => Some(2112),
        (12, 0b000000011100) => Some(2176),
        (12, 0b000000011101) => Some(2240),
        (12, 0b000000011110) => Some(2304),
        (12, 0b000000011111) => Some(2368),
        (12, 0b000000010000) => Some(2432),
        (12, 0b000000010001) => Some(2496),
        (12, 0b000000000001) => Some(0xFFFF), // EOL marker, treat as end
        _ => None,
    }
}

/// Match black Huffman code (ITU-T T.4 Tables 2 & 3).
fn match_black_code(bits: u32, len: u32) -> Option<usize> {
    match (len, bits) {
        (2, 0b11) => Some(2),
        (2, 0b10) => Some(3),
        (3, 0b010) => Some(1),
        (3, 0b011) => Some(4),
        (4, 0b0011) => Some(5),
        (4, 0b0010) => Some(6),
        (5, 0b00011) => Some(7),
        (6, 0b000101) => Some(8),
        (6, 0b000100) => Some(9),
        (7, 0b0000100) => Some(10),
        (7, 0b0000101) => Some(11),
        (7, 0b0000111) => Some(12),
        (8, 0b00000100) => Some(13),
        (8, 0b00000111) => Some(14),
        (9, 0b000011000) => Some(15),
        (10, 0b0000010111) => Some(0),
        (10, 0b0000011000) => Some(16),
        (10, 0b0000001000) => Some(17),
        (10, 0b0000110111) => Some(128), // makeup
        (11, 0b00000011001) => Some(18),
        (11, 0b00001100111) => Some(19),
        (11, 0b00001101000) => Some(20),
        (11, 0b00001101100) => Some(21),
        (11, 0b00000110111) => Some(22),
        (11, 0b00000101000) => Some(23),
        (11, 0b00000010111) => Some(24),
        (11, 0b00000011000) => Some(25),
        (11, 0b00001101011) => Some(192), // makeup
        // (11, 0b00001101100) already matched above for run=21
        (12, 0b000011001000) => Some(26),
        (12, 0b000011001001) => Some(27),
        (12, 0b000001101010) => Some(28),
        (12, 0b000001101011) => Some(29),
        (12, 0b000011010010) => Some(30),
        (12, 0b000011010011) => Some(31),
        (12, 0b000011010100) => Some(32),
        (12, 0b000011010101) => Some(33),
        (12, 0b000011010110) => Some(34),
        (12, 0b000011010111) => Some(35),
        (12, 0b000011011000) => Some(36),
        (12, 0b000011011001) => Some(37),
        (12, 0b000011011010) => Some(38),
        (12, 0b000011011011) => Some(39),
        (12, 0b000001010100) => Some(40),
        (12, 0b000001010101) => Some(41),
        (12, 0b000001010110) => Some(42),
        (12, 0b000001010111) => Some(43),
        (12, 0b000001100100) => Some(44),
        (12, 0b000001100101) => Some(45),
        (12, 0b000001010010) => Some(46),
        (12, 0b000001010011) => Some(47),
        (12, 0b000000100100) => Some(48),
        (12, 0b000000110111) => Some(49),
        (12, 0b000000111000) => Some(50),
        (12, 0b000000100111) => Some(51),
        (12, 0b000000101000) => Some(52),
        (12, 0b000001011000) => Some(53),
        (12, 0b000001011001) => Some(54),
        (12, 0b000000101011) => Some(55),
        (12, 0b000000101100) => Some(56),
        (12, 0b000001011010) => Some(57),
        (12, 0b000001100110) => Some(58),
        (12, 0b000001100111) => Some(59),
        (12, 0b000001101000) => Some(60),
        (12, 0b000001101001) => Some(61),
        // (12, 0b000001101010) already matched above for run=28
        // (12, 0b000001101011) already matched above for run=29
        (13, 0b0000001100100) => Some(62),
        (13, 0b0000001100101) => Some(63),
        // Black makeup codes
        (10, 0b0000001111) => Some(64),
        // (12, 0b000011001000) already matched above for run=26
        (12, 0b000011001010) => Some(256), // makeup
        (12, 0b000011001011) => Some(320), // makeup
        (12, 0b000011001100) => Some(384), // makeup
        (12, 0b000011001101) => Some(448), // makeup
        (13, 0b0000001101100) => Some(512), // makeup
        (13, 0b0000001101101) => Some(576), // makeup
        (13, 0b0000001001010) => Some(640), // makeup
        (13, 0b0000001001011) => Some(704), // makeup
        (13, 0b0000001001100) => Some(768), // makeup
        (13, 0b0000001001101) => Some(832), // makeup
        (13, 0b0000001110010) => Some(896), // makeup
        (13, 0b0000001110011) => Some(960), // makeup
        (13, 0b0000001110100) => Some(1024), // makeup
        (13, 0b0000001110101) => Some(1088), // makeup
        (13, 0b0000001110110) => Some(1152), // makeup
        (13, 0b0000001110111) => Some(1216), // makeup
        (13, 0b0000001010010) => Some(1280), // makeup
        (13, 0b0000001010011) => Some(1344), // makeup
        (13, 0b0000001010100) => Some(1408), // makeup
        (13, 0b0000001010101) => Some(1472), // makeup
        (13, 0b0000001011010) => Some(1536), // makeup
        (13, 0b0000001011011) => Some(1600), // makeup
        (11, 0b00000001000) => Some(1664), // makeup
        (12, 0b000000010010) => Some(1792), // makeup (shared)
        (12, 0b000000010011) => Some(1856),
        (12, 0b000000010100) => Some(1920),
        (12, 0b000000010101) => Some(1984),
        (12, 0b000000010110) => Some(2048),
        (12, 0b000000010111) => Some(2112),
        (12, 0b000000011100) => Some(2176),
        (12, 0b000000011101) => Some(2240),
        (12, 0b000000011110) => Some(2304),
        (12, 0b000000011111) => Some(2368),
        (12, 0b000000010000) => Some(2432),
        (12, 0b000000010001) => Some(2496),
        (12, 0b000000000001) => Some(0xFFFF), // EOL
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// EOL handling
// ---------------------------------------------------------------------------

fn skip_eol(reader: &mut BitReader) {
    // EOL = 000000000001 (12 bits: eleven 0s followed by a 1)
    // Try to find and skip it
    let mut zeros = 0;
    for _ in 0..24 {
        match reader.peek_bit() {
            Some(0) => {
                reader.read_bit();
                zeros += 1;
            }
            Some(1) => {
                if zeros >= 11 {
                    reader.read_bit(); // consume the 1
                }
                return;
            }
            _ => return,
        }
    }
}

// ---------------------------------------------------------------------------
// Bit reader
// ---------------------------------------------------------------------------

struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8, // 0-7, MSB first (7 = MSB)
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    fn is_eof(&self) -> bool {
        self.byte_pos >= self.data.len()
    }

    fn read_bit(&mut self) -> Option<u8> {
        if self.byte_pos >= self.data.len() {
            return None;
        }
        let bit = (self.data[self.byte_pos] >> (7 - self.bit_pos)) & 1;
        self.bit_pos += 1;
        if self.bit_pos >= 8 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
        Some(bit)
    }

    fn peek_bit(&self) -> Option<u8> {
        if self.byte_pos >= self.data.len() {
            return None;
        }
        Some((self.data[self.byte_pos] >> (7 - self.bit_pos)) & 1)
    }

    fn peek_bits(&self, count: u32) -> Option<u32> {
        let mut val = 0u32;
        let mut bp = self.byte_pos;
        let mut bi = self.bit_pos;

        for _ in 0..count {
            if bp >= self.data.len() {
                return None;
            }
            val = (val << 1) | ((self.data[bp] >> (7 - bi)) & 1) as u32;
            bi += 1;
            if bi >= 8 {
                bi = 0;
                bp += 1;
            }
        }
        Some(val)
    }

    fn align_byte(&mut self) {
        if self.bit_pos > 0 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_params_default() {
        let p = CcittParams::from_dict(None);
        assert_eq!(p.k, 0);
        assert_eq!(p.columns, 1728);
        assert!(!p.black_is1);
        assert!(p.end_of_block);
    }

    #[test]
    fn test_bit_reader_basic() {
        let data = [0b10110010, 0b01010101];
        let mut reader = BitReader::new(&data);
        assert_eq!(reader.read_bit(), Some(1));
        assert_eq!(reader.read_bit(), Some(0));
        assert_eq!(reader.read_bit(), Some(1));
        assert_eq!(reader.read_bit(), Some(1));
        assert_eq!(reader.read_bit(), Some(0));
        assert_eq!(reader.read_bit(), Some(0));
        assert_eq!(reader.read_bit(), Some(1));
        assert_eq!(reader.read_bit(), Some(0));
        // second byte
        assert_eq!(reader.read_bit(), Some(0));
        assert_eq!(reader.read_bit(), Some(1));
    }

    #[test]
    fn test_bit_reader_eof() {
        let data = [0xFF];
        let mut reader = BitReader::new(&data);
        for _ in 0..8 {
            assert!(reader.read_bit().is_some());
        }
        assert!(reader.read_bit().is_none());
        assert!(reader.is_eof());
    }

    #[test]
    fn test_decode_empty() {
        let result = decode(&[], None).unwrap();
        // columns=1728, rows=0 → empty output
        assert!(result.is_empty());
    }

    #[test]
    fn test_white_code_lookup() {
        // White code for run length 0: 00110101 (8 bits)
        assert_eq!(match_white_code(0b00110101, 8), Some(0));
        // White code for run length 1: 000111 (6 bits)
        assert_eq!(match_white_code(0b000111, 6), Some(1));
        // White code for run length 2: 0111 (4 bits)
        assert_eq!(match_white_code(0b0111, 4), Some(2));
    }

    #[test]
    fn test_black_code_lookup() {
        // Black code for run length 0: 0000010111 (10 bits)
        assert_eq!(match_black_code(0b0000010111, 10), Some(0));
        // Black code for run length 1: 010 (3 bits)
        assert_eq!(match_black_code(0b010, 3), Some(1));
        // Black code for run length 2: 11 (2 bits)
        assert_eq!(match_black_code(0b11, 2), Some(2));
    }

    #[test]
    fn test_group4_all_white_line() {
        // Group 4 encoding of a single all-white line (8 pixels):
        // All white means V(0) codes for each changing element on ref (all-white ref)
        // Since ref is all white and line is all white: no changing elements needed
        // Just EOFB (two EOLs): 000000000001 000000000001

        // For a truly blank page, Group 4 just needs EOFB
        let mut params = PdfDict::new();
        params.insert(
            b"K".to_vec(),
            crate::object::PdfObject::Integer(-1),
        );
        params.insert(
            b"Columns".to_vec(),
            crate::object::PdfObject::Integer(8),
        );
        params.insert(
            b"Rows".to_vec(),
            crate::object::PdfObject::Integer(1),
        );
        params.insert(
            b"EndOfBlock".to_vec(),
            crate::object::PdfObject::Bool(false),
        );

        // Encode: V(0) = 1 bit → the line is same as reference (all white)
        // We need at least 1 V(0) to indicate "no changes" then the line ends at columns
        // Actually for Group 4 all-white: a0 starts at -1, is_white=true
        // b1 on ref (all white) = columns, so V(0) sets a1=columns → line done
        // Encoding: single bit 1 (V(0))
        let data = [0b10000000]; // V(0) = 1, then padding

        let result = decode(&data, Some(&params)).unwrap();
        assert_eq!(result.len(), 8);
        // All white
        for &b in &result {
            assert_eq!(b, 0xFF);
        }
    }
}
