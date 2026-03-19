//! CFF (Compact Font Format) parser for Type1C fonts embedded in PDF.
//!
//! Parses the CFF binary format as specified in Adobe Technical Note #5176.
//! CFF data appears in PDF as `/FontFile3` streams with subtype `/Type1C`.
//!
//! This module parses the font structure (headers, dictionaries, charsets)
//! but does not interpret Type 2 CharString programs.

/// A parsed CFF font.
#[derive(Debug, Clone)]
pub struct CffFont {
    pub name: String,
    pub top_dict: CffTopDict,
    pub charset: CffCharset,
    pub char_strings_count: u32,
    pub default_width: f64,
    pub nominal_width: f64,
}

/// CFF Top DICT operator values.
#[derive(Debug, Clone, Default)]
pub struct CffTopDict {
    pub version: Option<String>,
    pub notice: Option<String>,
    pub full_name: Option<String>,
    pub family_name: Option<String>,
    pub weight: Option<String>,
    pub font_bbox: [f64; 4],
    pub charset_offset: u32,
    pub encoding_offset: u32,
    pub char_strings_offset: u32,
    pub private_offset: u32,
    pub private_size: u32,
    pub is_cid_font: bool,
    /// Registry, Ordering, Supplement for CID fonts.
    pub ros: Option<(String, String, i64)>,
    pub fd_array_offset: Option<u32>,
    pub fd_select_offset: Option<u32>,
}

/// CFF charset (glyph name mapping).
#[derive(Debug, Clone)]
pub enum CffCharset {
    /// Predefined ISO Adobe charset (offset 0).
    ISOAdobe,
    /// Predefined Expert charset (offset 1).
    Expert,
    /// Predefined Expert Subset charset (offset 2).
    ExpertSubset,
    /// Custom charset: SID array, one per glyph starting from glyph 1.
    Custom(Vec<u16>),
}

/// Parse a CFF font from raw bytes.
///
/// Returns `None` if the data is too short or structurally invalid.
pub fn parse_cff(data: &[u8]) -> Option<CffFont> {
    let mut r = Reader::new(data);

    // 1. Header
    let header = r.parse_header()?;

    // Skip to end of header (hdr_size may be > 4).
    r.pos = header.hdr_size as usize;

    // 2. Name INDEX
    let name_index = r.parse_index()?;
    let name = if name_index.is_empty() {
        String::new()
    } else {
        String::from_utf8_lossy(name_index.first().unwrap()).into_owned()
    };

    // 3. Top DICT INDEX
    let top_dict_index = r.parse_index()?;
    if top_dict_index.is_empty() {
        return None;
    }

    // 4. String INDEX
    let string_index = r.parse_index()?;

    // 5. Global Subr INDEX (skip it, we don't need subroutines)
    let _global_subr_index = r.parse_index()?;

    // Parse Top DICT
    let top_dict_data = &top_dict_index[0];
    let raw_top = parse_dict(top_dict_data)?;
    let top_dict = build_top_dict(&raw_top, &string_index);

    // 6. CharStrings INDEX (count only)
    let char_strings_count = if top_dict.char_strings_offset > 0 {
        let cs_offset = top_dict.char_strings_offset as usize;
        if cs_offset >= data.len() {
            return None;
        }
        let mut cs_r = Reader::new(data);
        cs_r.pos = cs_offset;
        let cs_index = cs_r.parse_index()?;
        cs_index.len() as u32
    } else {
        return None; // CharStrings is required
    };

    // 7. Private DICT -> widths
    let (default_width, nominal_width) =
        if top_dict.private_size > 0 && top_dict.private_offset > 0 {
            let priv_start = top_dict.private_offset as usize;
            let priv_end = priv_start + top_dict.private_size as usize;
            if priv_end <= data.len() {
                let priv_data = &data[priv_start..priv_end];
                parse_private_dict(priv_data)
            } else {
                (0.0, 0.0)
            }
        } else {
            (0.0, 0.0)
        };

    // 8. Charset
    let charset = parse_charset(data, top_dict.charset_offset, char_strings_count);

    Some(CffFont {
        name,
        top_dict,
        charset,
        char_strings_count,
        default_width,
        nominal_width,
    })
}

// ---------------------------------------------------------------------------
// Internal reader
// ---------------------------------------------------------------------------

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

struct CffHeader {
    #[allow(dead_code)]
    major: u8,
    #[allow(dead_code)]
    minor: u8,
    hdr_size: u8,
    #[allow(dead_code)]
    off_size: u8,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn read_u8(&mut self) -> Option<u8> {
        if self.pos < self.data.len() {
            let v = self.data[self.pos];
            self.pos += 1;
            Some(v)
        } else {
            None
        }
    }

    fn read_u16(&mut self) -> Option<u16> {
        if self.pos + 2 <= self.data.len() {
            let v = u16::from_be_bytes([self.data[self.pos], self.data[self.pos + 1]]);
            self.pos += 2;
            Some(v)
        } else {
            None
        }
    }

    fn read_offset(&mut self, off_size: u8) -> Option<u32> {
        if self.remaining() < off_size as usize {
            return None;
        }
        let mut val: u32 = 0;
        for _ in 0..off_size {
            val = (val << 8) | self.data[self.pos] as u32;
            self.pos += 1;
        }
        Some(val)
    }

    fn parse_header(&mut self) -> Option<CffHeader> {
        if self.remaining() < 4 {
            return None;
        }
        let major = self.read_u8()?;
        let minor = self.read_u8()?;
        let hdr_size = self.read_u8()?;
        let off_size = self.read_u8()?;
        if major != 1 || hdr_size < 4 || off_size == 0 || off_size > 4 {
            return None;
        }
        Some(CffHeader {
            major,
            minor,
            hdr_size,
            off_size,
        })
    }

    /// Parse a CFF INDEX structure and return the contained data items.
    fn parse_index(&mut self) -> Option<Vec<Vec<u8>>> {
        let count = self.read_u16()? as usize;
        if count == 0 {
            return Some(Vec::new());
        }
        let off_size = self.read_u8()?;
        if off_size == 0 || off_size > 4 {
            return None;
        }

        // Read count+1 offsets (1-based).
        let mut offsets = Vec::with_capacity(count + 1);
        for _ in 0..=count {
            offsets.push(self.read_offset(off_size)?);
        }

        // Data starts right after offsets; offsets are 1-based.
        let data_start = self.pos;
        let last_offset = *offsets.last().unwrap() as usize;
        if last_offset == 0 {
            return None;
        }
        let data_end = data_start + last_offset - 1;
        if data_end > self.data.len() {
            return None;
        }

        let mut items = Vec::with_capacity(count);
        for i in 0..count {
            let start = data_start + offsets[i] as usize - 1;
            let end = data_start + offsets[i + 1] as usize - 1;
            if end > self.data.len() || start > end {
                return None;
            }
            items.push(self.data[start..end].to_vec());
        }

        self.pos = data_end;
        Some(items)
    }
}

// ---------------------------------------------------------------------------
// DICT parsing
// ---------------------------------------------------------------------------

/// A raw DICT entry: operator -> operand stack at the time of the operator.
#[derive(Debug, Clone)]
struct DictEntry {
    operator: u16, // single-byte ops as-is; two-byte (12,X) encoded as 0x0C00 | X
    operands: Vec<DictOperand>,
}

#[derive(Debug, Clone, Copy)]
enum DictOperand {
    Integer(i64),
    Real(f64),
}

impl DictOperand {
    fn as_f64(self) -> f64 {
        match self {
            Self::Integer(i) => i as f64,
            Self::Real(f) => f,
        }
    }

    fn as_i64(self) -> i64 {
        match self {
            Self::Integer(i) => i,
            Self::Real(f) => f as i64,
        }
    }
}

/// Parse a DICT (sequence of operands followed by operators).
fn parse_dict(data: &[u8]) -> Option<Vec<DictEntry>> {
    let mut entries = Vec::new();
    let mut operands: Vec<DictOperand> = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        let b0 = data[pos];

        match b0 {
            // Operators
            0..=11 | 13..=21 => {
                entries.push(DictEntry {
                    operator: b0 as u16,
                    operands: std::mem::take(&mut operands),
                });
                pos += 1;
            }
            12 => {
                // Two-byte operator
                pos += 1;
                if pos >= data.len() {
                    return None;
                }
                let b1 = data[pos];
                entries.push(DictEntry {
                    operator: 0x0C00 | b1 as u16,
                    operands: std::mem::take(&mut operands),
                });
                pos += 1;
            }
            // Operands
            28 => {
                // 2-byte integer
                if pos + 2 >= data.len() {
                    return None;
                }
                let v = i16::from_be_bytes([data[pos + 1], data[pos + 2]]) as i64;
                operands.push(DictOperand::Integer(v));
                pos += 3;
            }
            29 => {
                // 4-byte integer
                if pos + 4 >= data.len() {
                    return None;
                }
                let v = i32::from_be_bytes([
                    data[pos + 1],
                    data[pos + 2],
                    data[pos + 3],
                    data[pos + 4],
                ]) as i64;
                operands.push(DictOperand::Integer(v));
                pos += 5;
            }
            30 => {
                // Real number (BCD nibble-encoded)
                pos += 1;
                let (val, new_pos) = parse_real_operand(data, pos)?;
                operands.push(DictOperand::Real(val));
                pos = new_pos;
            }
            32..=246 => {
                // 1-byte integer: value = b0 - 139
                operands.push(DictOperand::Integer(b0 as i64 - 139));
                pos += 1;
            }
            247..=250 => {
                // 2-byte positive: (b0 - 247) * 256 + b1 + 108
                if pos + 1 >= data.len() {
                    return None;
                }
                let b1 = data[pos + 1] as i64;
                let v = (b0 as i64 - 247) * 256 + b1 + 108;
                operands.push(DictOperand::Integer(v));
                pos += 2;
            }
            251..=254 => {
                // 2-byte negative: -(b0 - 251) * 256 - b1 - 108
                if pos + 1 >= data.len() {
                    return None;
                }
                let b1 = data[pos + 1] as i64;
                let v = -(b0 as i64 - 251) * 256 - b1 - 108;
                operands.push(DictOperand::Integer(v));
                pos += 2;
            }
            _ => {
                // Unknown byte (22-27, 31, 255 in DICT context) — skip
                pos += 1;
            }
        }
    }

    Some(entries)
}

/// Parse a BCD-encoded real number starting at `pos`.
/// Returns (value, new_pos).
fn parse_real_operand(data: &[u8], start: usize) -> Option<(f64, usize)> {
    let mut s = String::new();
    let mut pos = start;

    loop {
        if pos >= data.len() {
            return None;
        }
        let byte = data[pos];
        pos += 1;

        for &nibble in &[byte >> 4, byte & 0x0F] {
            match nibble {
                0..=9 => s.push((b'0' + nibble) as char),
                0xA => s.push('.'),
                0xB => s.push('E'),
                0xC => s.push_str("E-"),
                0xD => {
                    // Reserved — skip
                }
                0xE => s.push('-'),
                0xF => return s.parse::<f64>().ok().map(|v| (v, pos)),
                _ => unreachable!(),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Building typed dictionaries from raw entries
// ---------------------------------------------------------------------------

/// Resolve a SID to a string, using the standard strings + custom string index.
fn sid_to_string(sid: u16, string_index: &[Vec<u8>]) -> String {
    // CFF defines 391 standard strings (SIDs 0..390).
    if (sid as usize) < STANDARD_STRINGS.len() {
        return STANDARD_STRINGS[sid as usize].to_string();
    }
    let custom_idx = sid as usize - STANDARD_STRINGS.len();
    if custom_idx < string_index.len() {
        String::from_utf8_lossy(&string_index[custom_idx]).into_owned()
    } else {
        String::new()
    }
}

fn build_top_dict(entries: &[DictEntry], string_index: &[Vec<u8>]) -> CffTopDict {
    let mut d = CffTopDict::default();

    for entry in entries {
        match entry.operator {
            // version (SID)
            0 => {
                if let Some(op) = entry.operands.first() {
                    d.version = Some(sid_to_string(op.as_i64() as u16, string_index));
                }
            }
            // Notice (SID)
            1 => {
                if let Some(op) = entry.operands.first() {
                    d.notice = Some(sid_to_string(op.as_i64() as u16, string_index));
                }
            }
            // FullName (SID)
            2 => {
                if let Some(op) = entry.operands.first() {
                    d.full_name = Some(sid_to_string(op.as_i64() as u16, string_index));
                }
            }
            // FamilyName (SID)
            3 => {
                if let Some(op) = entry.operands.first() {
                    d.family_name = Some(sid_to_string(op.as_i64() as u16, string_index));
                }
            }
            // Weight (SID)
            4 => {
                if let Some(op) = entry.operands.first() {
                    d.weight = Some(sid_to_string(op.as_i64() as u16, string_index));
                }
            }
            // FontBBox
            5 => {
                if entry.operands.len() >= 4 {
                    d.font_bbox = [
                        entry.operands[0].as_f64(),
                        entry.operands[1].as_f64(),
                        entry.operands[2].as_f64(),
                        entry.operands[3].as_f64(),
                    ];
                }
            }
            // charset offset
            15 => {
                if let Some(op) = entry.operands.first() {
                    d.charset_offset = op.as_i64() as u32;
                }
            }
            // Encoding offset
            16 => {
                if let Some(op) = entry.operands.first() {
                    d.encoding_offset = op.as_i64() as u32;
                }
            }
            // CharStrings offset
            17 => {
                if let Some(op) = entry.operands.first() {
                    d.char_strings_offset = op.as_i64() as u32;
                }
            }
            // Private (size, offset)
            18 => {
                if entry.operands.len() >= 2 {
                    d.private_size = entry.operands[0].as_i64() as u32;
                    d.private_offset = entry.operands[1].as_i64() as u32;
                }
            }
            // Two-byte operators (12, X)
            op if op & 0xFF00 == 0x0C00 => {
                let sub = (op & 0xFF) as u8;
                match sub {
                    // ROS (Registry SID, Ordering SID, Supplement)
                    30 => {
                        if entry.operands.len() >= 3 {
                            let registry =
                                sid_to_string(entry.operands[0].as_i64() as u16, string_index);
                            let ordering =
                                sid_to_string(entry.operands[1].as_i64() as u16, string_index);
                            let supplement = entry.operands[2].as_i64();
                            d.ros = Some((registry, ordering, supplement));
                            d.is_cid_font = true;
                        }
                    }
                    // FDArray
                    36 => {
                        if let Some(op) = entry.operands.first() {
                            d.fd_array_offset = Some(op.as_i64() as u32);
                        }
                    }
                    // FDSelect
                    37 => {
                        if let Some(op) = entry.operands.first() {
                            d.fd_select_offset = Some(op.as_i64() as u32);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    d
}

/// Parse the Private DICT and extract (defaultWidthX, nominalWidthX).
fn parse_private_dict(data: &[u8]) -> (f64, f64) {
    let entries = match parse_dict(data) {
        Some(e) => e,
        None => return (0.0, 0.0),
    };

    let mut default_width = 0.0;
    let mut nominal_width = 0.0;

    for entry in &entries {
        match entry.operator {
            20 => {
                if let Some(op) = entry.operands.first() {
                    default_width = op.as_f64();
                }
            }
            21 => {
                if let Some(op) = entry.operands.first() {
                    nominal_width = op.as_f64();
                }
            }
            _ => {}
        }
    }

    (default_width, nominal_width)
}

// ---------------------------------------------------------------------------
// Charset parsing
// ---------------------------------------------------------------------------

fn parse_charset(data: &[u8], offset: u32, num_glyphs: u32) -> CffCharset {
    match offset {
        0 => CffCharset::ISOAdobe,
        1 => CffCharset::Expert,
        2 => CffCharset::ExpertSubset,
        _ => parse_custom_charset(data, offset as usize, num_glyphs),
    }
}

fn parse_custom_charset(data: &[u8], offset: usize, num_glyphs: u32) -> CffCharset {
    if offset >= data.len() {
        return CffCharset::ISOAdobe; // fallback
    }

    let format = data[offset];
    let mut sids = Vec::new();
    // .notdef (glyph 0) is implicit; charset covers glyphs 1..num_glyphs-1.
    let remaining = (num_glyphs as usize).saturating_sub(1);
    let mut pos = offset + 1;

    match format {
        0 => {
            // Format 0: array of SIDs
            for _ in 0..remaining {
                if pos + 2 > data.len() {
                    break;
                }
                let sid = u16::from_be_bytes([data[pos], data[pos + 1]]);
                sids.push(sid);
                pos += 2;
            }
        }
        1 => {
            // Format 1: ranges with 1-byte count
            while sids.len() < remaining {
                if pos + 3 > data.len() {
                    break;
                }
                let first = u16::from_be_bytes([data[pos], data[pos + 1]]);
                let n_left = data[pos + 2] as u16;
                pos += 3;
                for i in 0..=n_left {
                    if sids.len() >= remaining {
                        break;
                    }
                    sids.push(first + i);
                }
            }
        }
        2 => {
            // Format 2: ranges with 2-byte count
            while sids.len() < remaining {
                if pos + 4 > data.len() {
                    break;
                }
                let first = u16::from_be_bytes([data[pos], data[pos + 1]]);
                let n_left = u16::from_be_bytes([data[pos + 2], data[pos + 3]]);
                pos += 4;
                for i in 0..=n_left {
                    if sids.len() >= remaining {
                        break;
                    }
                    sids.push(first + i);
                }
            }
        }
        _ => {
            return CffCharset::ISOAdobe; // unknown format, fallback
        }
    }

    CffCharset::Custom(sids)
}

// ---------------------------------------------------------------------------
// Standard CFF strings (first 391 SIDs)
// ---------------------------------------------------------------------------

/// The 391 predefined standard strings in CFF (SIDs 0-390).
/// Only the first entries are included here for practical use; the rest map to
/// glyph names from the Adobe standard set.
#[rustfmt::skip]
static STANDARD_STRINGS: &[&str] = &[
    // 0-9
    ".notdef", "space", "exclam", "quotedbl", "numbersign",
    "dollar", "percent", "ampersand", "quoteright", "parenleft",
    // 10-19
    "parenright", "asterisk", "plus", "comma", "hyphen",
    "period", "slash", "zero", "one", "two",
    // 20-29
    "three", "four", "five", "six", "seven",
    "eight", "nine", "colon", "semicolon", "less",
    // 30-39
    "equal", "greater", "question", "at", "A",
    "B", "C", "D", "E", "F",
    // 40-49
    "G", "H", "I", "J", "K",
    "L", "M", "N", "O", "P",
    // 50-59
    "Q", "R", "S", "T", "U",
    "V", "W", "X", "Y", "Z",
    // 60-69
    "bracketleft", "backslash", "bracketright", "asciicircum", "underscore",
    "quoteleft", "a", "b", "c", "d",
    // 70-79
    "e", "f", "g", "h", "i",
    "j", "k", "l", "m", "n",
    // 80-89
    "o", "p", "q", "r", "s",
    "t", "u", "v", "w", "x",
    // 90-99
    "y", "z", "braceleft", "bar", "braceright",
    "asciitilde", "exclamdown", "cent", "sterling", "fraction",
    // 100-109
    "yen", "florin", "section", "currency", "quotesingle",
    "quotedblleft", "guillemotleft", "guilsinglleft", "guilsinglright", "fi",
    // 110-119
    "fl", "endash", "dagger", "daggerdbl", "periodcentered",
    "paragraph", "bullet", "quotesinglbase", "quotedblbase", "quotedblright",
    // 120-129
    "guillemotright", "ellipsis", "perthousand", "questiondown", "grave",
    "acute", "circumflex", "tilde", "macron", "breve",
    // 130-139
    "dotaccent", "dieresis", "ring", "cedilla", "hungarumlaut",
    "ogonek", "caron", "emdash", "AE", "ordfeminine",
    // 140-149
    "Lslash", "Oslash", "OE", "ordmasculine", "ae",
    "dotlessi", "lslash", "oslash", "oe", "germandbls",
    // 150-159
    "onesuperior", "logicalnot", "mu", "trademark", "Eth",
    "onehalf", "plusminus", "Thorn", "onequarter", "divide",
    // 160-169
    "brokenbar", "degree", "thorn", "threequarters", "twosuperior",
    "registered", "minus", "eth", "multiply", "threesuperior",
    // 170-179
    "copyright", "Aacute", "Acircumflex", "Adieresis", "Agrave",
    "Aring", "Atilde", "Ccedilla", "Eacute", "Ecircumflex",
    // 180-189
    "Edieresis", "Egrave", "Iacute", "Icircumflex", "Idieresis",
    "Igrave", "Ntilde", "Oacute", "Ocircumflex", "Odieresis",
    // 190-199
    "Ograve", "Otilde", "Scaron", "Uacute", "Ucircumflex",
    "Udieresis", "Ugrave", "Yacute", "Ydieresis", "Zcaron",
    // 200-209
    "aacute", "acircumflex", "adieresis", "agrave", "aring",
    "atilde", "ccedilla", "eacute", "ecircumflex", "edieresis",
    // 210-219
    "egrave", "iacute", "icircumflex", "idieresis", "igrave",
    "ntilde", "oacute", "ocircumflex", "odieresis", "ograve",
    // 220-229
    "otilde", "scaron", "uacute", "ucircumflex", "udieresis",
    "ugrave", "yacute", "ydieresis", "zcaron", "exclamsmall",
    // 230-239
    "Hungarumlautsmall", "dollaroldstyle", "dollarsuperior", "ampersandsmall",
    "Acutesmall", "parenleftsuperior", "parenrightsuperior", "twodotenleader",
    "onedotenleader", "zerooldstyle",
    // 240-249
    "oneoldstyle", "twooldstyle", "threeoldstyle", "fouroldstyle",
    "fiveoldstyle", "sixoldstyle", "sevenoldstyle", "eightoldstyle",
    "nineoldstyle", "commasuperior",
    // 250-259
    "threequartersemdash", "periodsuperior", "questionsmall", "asuperior",
    "bsuperior", "centsuperior", "dsuperior", "esuperior", "isuperior",
    "lsuperior",
    // 260-269
    "msuperior", "nsuperior", "osuperior", "rsuperior", "ssuperior",
    "tsuperior", "ff", "ffi", "ffl", "parenleftinferior",
    // 270-279
    "parenrightinferior", "Circumflexsmall", "hyphensuperior", "Gravesmall",
    "Asmall", "Bsmall", "Csmall", "Dsmall", "Esmall", "Fsmall",
    // 280-289
    "Gsmall", "Hsmall", "Ismall", "Jsmall", "Ksmall",
    "Lsmall", "Msmall", "Nsmall", "Osmall", "Psmall",
    // 290-299
    "Qsmall", "Rsmall", "Ssmall", "Tsmall", "Usmall",
    "Vsmall", "Wsmall", "Xsmall", "Ysmall", "Zsmall",
    // 300-309
    "colonmonetary", "onefitted", "rupiah", "Tildesmall", "exclamdownsmall",
    "centoldstyle", "Lslashsmall", "Scaronsmall", "Zcaronsmall", "Dieresissmall",
    // 310-319
    "Brevesmall", "Caronsmall", "Dotaccentsmall", "Macronsmall", "figuredash",
    "hypheninferior", "Ogoneksmall", "Ringsmall", "Cedillasmall", "questiondownsmall",
    // 320-329
    "oneeighth", "threeeighths", "fiveeighths", "seveneighths", "onethird",
    "twothirds", "zerosuperior", "foursuperior", "fivesuperior", "sixsuperior",
    // 330-339
    "sevensuperior", "eightsuperior", "ninesuperior", "zeroinferior", "oneinferior",
    "twoinferior", "threeinferior", "fourinferior", "fiveinferior", "sixinferior",
    // 340-349
    "seveninferior", "eightinferior", "nineinferior", "centinferior", "dollarinferior",
    "periodinferior", "commainferior", "Agravesmall", "Aacutesmall", "Acircumflexsmall",
    // 350-359
    "Atildesmall", "Adieresissmall", "Aringsmall", "AEsmall", "Ccedillasmall",
    "Egravesmall", "Eacutesmall", "Ecircumflexsmall", "Edieresissmall", "Igravesmall",
    // 360-369
    "Iacutesmall", "Icircumflexsmall", "Idieresissmall", "Ethsmall", "Ntildesmall",
    "Ogravesmall", "Oacutesmall", "Ocircumflexsmall", "Otildesmall", "Odieresissmall",
    // 370-379
    "OEsmall", "Oslashsmall", "Ugravesmall", "Uacutesmall", "Ucircumflexsmall",
    "Udieresissmall", "Yacutesmall", "Thornsmall", "Ydieresissmall",
    "001.000",
    // 380-389
    "001.001", "001.002", "001.003", "Black", "Bold",
    "Book", "Light", "Medium", "Regular", "Roman",
    // 390
    "Semibold",
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a CFF INDEX from a list of byte slices.
    fn build_index(items: &[&[u8]]) -> Vec<u8> {
        let count = items.len();
        let mut data_buf = Vec::new();
        let mut offsets = vec![1u32]; // 1-based
        for item in items {
            data_buf.extend_from_slice(item);
            offsets.push(data_buf.len() as u32 + 1);
        }

        // Determine off_size
        let max_off = *offsets.last().unwrap();
        let off_size: u8 = if max_off <= 0xFF {
            1
        } else if max_off <= 0xFFFF {
            2
        } else if max_off <= 0xFF_FFFF {
            3
        } else {
            4
        };

        let mut buf = Vec::new();
        buf.extend_from_slice(&(count as u16).to_be_bytes());
        buf.push(off_size);
        for o in &offsets {
            match off_size {
                1 => buf.push(*o as u8),
                2 => buf.extend_from_slice(&(*o as u16).to_be_bytes()),
                3 => {
                    buf.push((*o >> 16) as u8);
                    buf.push((*o >> 8) as u8);
                    buf.push(*o as u8);
                }
                4 => buf.extend_from_slice(&o.to_be_bytes()),
                _ => unreachable!(),
            }
        }
        buf.extend_from_slice(&data_buf);
        buf
    }

    /// Helper: build an empty INDEX (count=0).
    fn build_empty_index() -> Vec<u8> {
        vec![0, 0] // count = 0
    }

    /// Encode a DICT integer operand.
    fn encode_dict_int(val: i64) -> Vec<u8> {
        if (-107..=107).contains(&val) {
            vec![(val + 139) as u8]
        } else if (108..=1131).contains(&val) {
            let v = val - 108;
            vec![(v / 256 + 247) as u8, (v % 256) as u8]
        } else if (-1131..=-108).contains(&val) {
            let v = -val - 108;
            vec![(v / 256 + 251) as u8, (v % 256) as u8]
        } else if (-32768..=32767).contains(&val) {
            let mut buf = vec![28];
            buf.extend_from_slice(&(val as i16).to_be_bytes());
            buf
        } else {
            let mut buf = vec![29];
            buf.extend_from_slice(&(val as i32).to_be_bytes());
            buf
        }
    }

    /// Encode a DICT real operand.
    fn encode_dict_real(s: &str) -> Vec<u8> {
        let mut nibbles = Vec::new();
        for ch in s.chars() {
            match ch {
                '0'..='9' => nibbles.push(ch as u8 - b'0'),
                '.' => nibbles.push(0xA),
                'E' => nibbles.push(0xB),
                '-' => nibbles.push(0xE),
                _ => {}
            }
        }
        nibbles.push(0xF); // end marker

        // Pad to even number of nibbles
        if nibbles.len() % 2 != 0 {
            nibbles.push(0xF);
        }

        let mut buf = vec![30]; // real operand marker
        for chunk in nibbles.chunks(2) {
            buf.push((chunk[0] << 4) | chunk[1]);
        }
        buf
    }

    /// Build a minimal valid CFF with given top dict entries and a specific number
    /// of charstrings.
    fn build_minimal_cff(
        font_name: &str,
        top_dict_extra: &[u8],
        private_dict: &[u8],
        num_charstrings: usize,
    ) -> Vec<u8> {
        // We'll build the CFF in a buffer and fix up offsets.
        // Header: 4 bytes
        let header = vec![1, 0, 4, 1]; // major=1, minor=0, hdr_size=4, off_size=1

        // Name INDEX
        let name_index = build_index(&[font_name.as_bytes()]);

        // We need to calculate offsets for CharStrings and Private before
        // building the Top DICT, but the Top DICT size affects offsets...
        // So we do two passes.

        // String INDEX (empty for minimal)
        let string_index = build_empty_index();

        // Global Subr INDEX (empty)
        let global_subr_index = build_empty_index();

        // CharStrings INDEX: minimal charstring data (just endchar = 14)
        let charstring_data: Vec<u8> = vec![14]; // endchar
        let charstring_items: Vec<&[u8]> = (0..num_charstrings)
            .map(|_| charstring_data.as_slice())
            .collect();
        let charstrings_index = build_index(&charstring_items);

        // First pass: estimate top dict to get sizes
        // Offsets will be fixed in second pass.
        let base_offset = header.len() + name_index.len();
        // Top DICT INDEX overhead: count(2) + offsize(1) + 2 offsets(variable) + data
        // We'll just build it and measure.

        // Build top dict content with placeholder offsets
        // We'll use large int encoding (29 = 5 bytes) for offsets to keep size stable.
        fn encode_fixed_offset(val: u32) -> Vec<u8> {
            let mut buf = vec![29];
            buf.extend_from_slice(&(val as i32).to_be_bytes());
            buf
        }

        // Calculate where things will be after top dict index + string index + global subr index
        // top_dict_content_size depends on content. Let's estimate:
        // CharStrings op (17): 5 bytes offset + 1 byte op = 6
        // Private op (18): 5+5 bytes operands + 1 byte op = 11
        // extra: top_dict_extra.len()
        // Total content estimate: 6 + 11 + extra = 17 + extra
        let td_content_size = 6 + 11 + top_dict_extra.len();
        // INDEX overhead for 1 item: count(2) + offsize(1) + (count+1) offsets
        // offsize depends on data length: for small data (<= 255), offsize=1
        let off_size: usize = if td_content_size + 1 <= 0xFF { 1 } else { 2 };
        let td_index_overhead = 2 + 1 + 2 * off_size; // count + offsize_byte + 2 offsets
        let top_dict_index_size = td_index_overhead + td_content_size;

        let charstrings_offset =
            base_offset + top_dict_index_size + string_index.len() + global_subr_index.len();
        let private_offset = charstrings_offset + charstrings_index.len();
        let private_size = private_dict.len();

        // Now build the real top dict content
        let mut td_content = Vec::new();
        td_content.extend_from_slice(top_dict_extra);
        // CharStrings offset (op 17)
        td_content.extend_from_slice(&encode_fixed_offset(charstrings_offset as u32));
        td_content.push(17);
        // Private size, offset (op 18)
        td_content.extend_from_slice(&encode_fixed_offset(private_size as u32));
        td_content.extend_from_slice(&encode_fixed_offset(private_offset as u32));
        td_content.push(18);

        let top_dict_index = build_index(&[&td_content]);

        // Assemble
        let mut cff = Vec::new();
        cff.extend_from_slice(&header);
        cff.extend_from_slice(&name_index);
        cff.extend_from_slice(&top_dict_index);
        cff.extend_from_slice(&string_index);
        cff.extend_from_slice(&global_subr_index);
        cff.extend_from_slice(&charstrings_index);
        cff.extend_from_slice(private_dict);

        cff
    }

    #[test]
    fn test_parse_minimal_cff() {
        let cff = build_minimal_cff("TestFont", &[], &[], 3);
        let font = parse_cff(&cff).expect("should parse minimal CFF");
        assert_eq!(font.name, "TestFont");
        assert_eq!(font.char_strings_count, 3);
        assert_eq!(font.default_width, 0.0);
        assert_eq!(font.nominal_width, 0.0);
    }

    #[test]
    fn test_parse_cff_with_private_widths() {
        // Private DICT with defaultWidthX=250 (op 20) and nominalWidthX=300 (op 21)
        let mut private_dict = Vec::new();
        private_dict.extend_from_slice(&encode_dict_int(250));
        private_dict.push(20); // defaultWidthX
        private_dict.extend_from_slice(&encode_dict_int(300));
        private_dict.push(21); // nominalWidthX

        let cff = build_minimal_cff("WidthFont", &[], &private_dict, 2);
        let font = parse_cff(&cff).expect("should parse CFF with private dict");
        assert_eq!(font.default_width, 250.0);
        assert_eq!(font.nominal_width, 300.0);
    }

    #[test]
    fn test_index_parsing_offsize_1() {
        let index = build_index(&[b"abc", b"de"]);
        let mut r = Reader::new(&index);
        let items = r.parse_index().expect("should parse index");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], b"abc");
        assert_eq!(items[1], b"de");
    }

    #[test]
    fn test_index_parsing_offsize_2() {
        // Create data large enough to require offsize=2
        let big_item = vec![0xAA; 300];
        let index = build_index(&[&big_item]);
        let mut r = Reader::new(&index);
        let items = r.parse_index().expect("should parse index");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].len(), 300);
    }

    #[test]
    fn test_index_parsing_offsize_3() {
        // Offsize=3 requires offset > 0xFFFF
        let big_item = vec![0xBB; 70_000];
        let index = build_index(&[&big_item]);
        let mut r = Reader::new(&index);
        let items = r.parse_index().expect("should parse index offsize=3");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].len(), 70_000);
    }

    #[test]
    fn test_empty_index() {
        let index = build_empty_index();
        let mut r = Reader::new(&index);
        let items = r.parse_index().expect("should parse empty index");
        assert!(items.is_empty());
    }

    #[test]
    fn test_dict_integer_1byte() {
        // Value 0: encoded as 139
        let data = vec![139, 0]; // operand 0, operator 0 (version)
        let entries = parse_dict(&data).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].operands[0].as_i64(), 0);

        // Value 100: encoded as 239
        let data = vec![239, 0];
        let entries = parse_dict(&data).unwrap();
        assert_eq!(entries[0].operands[0].as_i64(), 100);

        // Value -107: encoded as 32
        let data = vec![32, 0];
        let entries = parse_dict(&data).unwrap();
        assert_eq!(entries[0].operands[0].as_i64(), -107);
    }

    #[test]
    fn test_dict_integer_2byte_positive() {
        // Value 108: b0=247, b1=0 -> (247-247)*256 + 0 + 108 = 108
        let data = vec![247, 0, 0]; // operand, then operator 0
        let entries = parse_dict(&data).unwrap();
        assert_eq!(entries[0].operands[0].as_i64(), 108);

        // Value 1131: b0=250, b1=255 -> (250-247)*256 + 255 + 108 = 768+255+108 = 1131
        let data = vec![250, 255, 0];
        let entries = parse_dict(&data).unwrap();
        assert_eq!(entries[0].operands[0].as_i64(), 1131);
    }

    #[test]
    fn test_dict_integer_2byte_negative() {
        // Value -108: b0=251, b1=0 -> -(251-251)*256 - 0 - 108 = -108
        let data = vec![251, 0, 0];
        let entries = parse_dict(&data).unwrap();
        assert_eq!(entries[0].operands[0].as_i64(), -108);

        // Value -1131: b0=254, b1=255 -> -(254-251)*256 - 255 - 108 = -768-255-108 = -1131
        let data = vec![254, 255, 0];
        let entries = parse_dict(&data).unwrap();
        assert_eq!(entries[0].operands[0].as_i64(), -1131);
    }

    #[test]
    fn test_dict_integer_3byte() {
        // 28 followed by 2 bytes (i16)
        // Value 1000: 0x03E8
        let data = vec![28, 0x03, 0xE8, 0]; // operand, operator 0
        let entries = parse_dict(&data).unwrap();
        assert_eq!(entries[0].operands[0].as_i64(), 1000);

        // Value -1000
        let data = vec![28, 0xFC, 0x18, 0]; // -1000 as i16 big-endian
        let entries = parse_dict(&data).unwrap();
        assert_eq!(entries[0].operands[0].as_i64(), -1000);
    }

    #[test]
    fn test_dict_integer_5byte() {
        // 29 followed by 4 bytes (i32)
        // Value 100000
        let data = vec![29, 0x00, 0x01, 0x86, 0xA0, 0]; // operand, operator 0
        let entries = parse_dict(&data).unwrap();
        assert_eq!(entries[0].operands[0].as_i64(), 100_000);

        // Value -100000
        let val = (-100_000i32).to_be_bytes();
        let data = vec![29, val[0], val[1], val[2], val[3], 0];
        let entries = parse_dict(&data).unwrap();
        assert_eq!(entries[0].operands[0].as_i64(), -100_000);
    }

    #[test]
    fn test_dict_real_decoding() {
        // Encode "3.14" -> nibbles: 3, A(.), 1, 4, F(end), F(pad)
        let data = vec![
            30, 0x3A, 0x14, 0xFF, // real: 3.14
            0,  // operator 0
        ];
        let entries = parse_dict(&data).unwrap();
        let val = entries[0].operands[0].as_f64();
        assert!((val - 3.14).abs() < 1e-10);
    }

    #[test]
    fn test_dict_real_negative() {
        // Encode "-2.5" -> nibbles: E(-), 2, A(.), 5, F(end), F(pad)
        let data = vec![
            30, 0xE2, 0xA5, 0xFF, // real: -2.5
            0,  // operator 0
        ];
        let entries = parse_dict(&data).unwrap();
        let val = entries[0].operands[0].as_f64();
        assert!((val - (-2.5)).abs() < 1e-10);
    }

    #[test]
    fn test_dict_real_scientific() {
        // Encode "1E3" = 1000.0 -> nibbles: 1, B(E), 3, F(end)
        let data = vec![
            30, 0x1B, 0x3F, // real: 1E3
            0, // operator 0
        ];
        let entries = parse_dict(&data).unwrap();
        let val = entries[0].operands[0].as_f64();
        assert!((val - 1000.0).abs() < 1e-10);
    }

    #[test]
    fn test_predefined_charsets() {
        assert!(matches!(parse_charset(&[], 0, 10), CffCharset::ISOAdobe));
        assert!(matches!(parse_charset(&[], 1, 10), CffCharset::Expert));
        assert!(matches!(
            parse_charset(&[], 2, 10),
            CffCharset::ExpertSubset
        ));
    }

    #[test]
    fn test_custom_charset_format0() {
        // Format 0: one SID per glyph (after .notdef)
        let mut data = vec![0u8; 100];
        let offset = 10;
        data[offset] = 0; // format 0
        // 3 glyphs total: .notdef + 2 custom
        data[offset + 1] = 0x00;
        data[offset + 2] = 0x05; // SID 5
        data[offset + 3] = 0x00;
        data[offset + 4] = 0x0A; // SID 10

        match parse_charset(&data, offset as u32, 3) {
            CffCharset::Custom(sids) => {
                assert_eq!(sids, vec![5, 10]);
            }
            _ => panic!("expected Custom charset"),
        }
    }

    #[test]
    fn test_header_parsing() {
        let data = vec![1, 0, 4, 1]; // valid header
        let mut r = Reader::new(&data);
        let h = r.parse_header().expect("should parse header");
        assert_eq!(h.major, 1);
        assert_eq!(h.hdr_size, 4);
    }

    #[test]
    fn test_header_too_short() {
        let data = vec![1, 0]; // only 2 bytes
        let mut r = Reader::new(&data);
        assert!(r.parse_header().is_none());
    }

    #[test]
    fn test_header_wrong_major() {
        let data = vec![2, 0, 4, 1]; // major=2, not supported
        let mut r = Reader::new(&data);
        assert!(r.parse_header().is_none());
    }

    #[test]
    fn test_invalid_data_returns_none() {
        assert!(parse_cff(&[]).is_none());
        assert!(parse_cff(&[1, 0]).is_none());
        assert!(parse_cff(&[0xFF, 0xFF, 0xFF, 0xFF]).is_none());
    }

    #[test]
    fn test_sid_to_string_standard() {
        let empty: Vec<Vec<u8>> = vec![];
        assert_eq!(sid_to_string(0, &empty), ".notdef");
        assert_eq!(sid_to_string(1, &empty), "space");
        assert_eq!(sid_to_string(390, &empty), "Semibold");
    }

    #[test]
    fn test_sid_to_string_custom() {
        let custom = vec![b"MyGlyph".to_vec(), b"Another".to_vec()];
        assert_eq!(sid_to_string(391, &custom), "MyGlyph");
        assert_eq!(sid_to_string(392, &custom), "Another");
        assert_eq!(sid_to_string(393, &custom), ""); // out of range
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        // Test that our test helpers produce values the parser decodes correctly.
        for val in &[0i64, 1, -1, 107, -107, 108, -108, 1131, -1131, 32767, -32768, 100_000] {
            let mut encoded = encode_dict_int(*val);
            encoded.push(0); // operator
            let entries = parse_dict(&encoded).unwrap();
            assert_eq!(
                entries[0].operands[0].as_i64(),
                *val,
                "roundtrip failed for {val}"
            );
        }
    }

    #[test]
    fn test_two_byte_operator() {
        // 12, 30 = ROS operator. Need 3 operands.
        let mut data = Vec::new();
        data.extend_from_slice(&encode_dict_int(391)); // registry SID
        data.extend_from_slice(&encode_dict_int(392)); // ordering SID
        data.extend_from_slice(&encode_dict_int(0)); // supplement
        data.push(12);
        data.push(30); // ROS operator

        let entries = parse_dict(&data).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].operator, 0x0C00 | 30);
        assert_eq!(entries[0].operands.len(), 3);
    }
}
