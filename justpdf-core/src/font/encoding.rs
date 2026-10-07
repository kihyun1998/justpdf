use super::agl::AGL;
use super::encoding_tables::{MAC_ROMAN_TO_UNICODE, STANDARD_TO_UNICODE};
use crate::object::PdfObject;

/// PDF text encoding type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    StandardEncoding,
    MacRomanEncoding,
    WinAnsiEncoding,
    PDFDocEncoding,
    /// Identity (pass-through, for CID fonts).
    Identity,
}

impl Encoding {
    pub fn from_name(name: &[u8]) -> Self {
        match name {
            b"StandardEncoding" => Self::StandardEncoding,
            b"MacRomanEncoding" => Self::MacRomanEncoding,
            b"WinAnsiEncoding" => Self::WinAnsiEncoding,
            b"PDFDocEncoding" => Self::PDFDocEncoding,
            b"Identity-H" | b"Identity-V" => Self::Identity,
            _ => Self::StandardEncoding,
        }
    }
}

/// Decode a PDF byte string to a Unicode string using the given encoding.
pub fn decode_text(bytes: &[u8], encoding: Encoding) -> String {
    // Check for UTF-16BE BOM
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        return decode_utf16be(&bytes[2..]);
    }

    // Check for UTF-8 BOM
    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        return String::from_utf8_lossy(&bytes[3..]).into_owned();
    }

    match encoding {
        Encoding::WinAnsiEncoding => decode_winansi(bytes),
        Encoding::MacRomanEncoding => decode_table(bytes, &MAC_ROMAN_TO_UNICODE),
        Encoding::PDFDocEncoding => decode_pdfdoc(bytes),
        Encoding::StandardEncoding => decode_table(bytes, &STANDARD_TO_UNICODE),
        Encoding::Identity => {
            // Try UTF-8 first
            String::from_utf8_lossy(bytes).into_owned()
        }
    }
}

fn decode_utf16be(bytes: &[u8]) -> String {
    let mut chars = Vec::new();
    let mut i = 0;
    while i + 1 < bytes.len() {
        let code = ((bytes[i] as u16) << 8) | bytes[i + 1] as u16;
        i += 2;

        // Handle surrogate pairs
        if (0xD800..=0xDBFF).contains(&code) && i + 1 < bytes.len() {
            let low = ((bytes[i] as u16) << 8) | bytes[i + 1] as u16;
            if (0xDC00..=0xDFFF).contains(&low) {
                i += 2;
                let cp = 0x10000 + ((code as u32 - 0xD800) << 10) + (low as u32 - 0xDC00);
                if let Some(c) = char::from_u32(cp) {
                    chars.push(c);
                }
                continue;
            }
        }

        if let Some(c) = char::from_u32(code as u32) {
            chars.push(c);
        }
    }
    chars.into_iter().collect()
}

/// WinAnsi (Windows-1252) decoding.
fn decode_winansi(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| WINANSI_TO_UNICODE[b as usize])
        .collect()
}

/// Decoding through a code-to-Unicode table; a code mapped to '\0' has no glyph and yields nothing.
fn decode_table(bytes: &[u8], table: &[char; 256]) -> String {
    bytes
        .iter()
        .map(|&b| table[b as usize])
        .filter(|&c| c != '\0')
        .collect()
}

/// The code-to-glyph-name pairs of an encoding dictionary's `/Differences` array
/// (`[code name name ... code name ...]`: each name takes the next code).
pub fn parse_differences(differences: &[PdfObject]) -> Vec<(u8, Vec<u8>)> {
    let mut result = Vec::new();
    let mut current_code: Option<u8> = None;
    for obj in differences {
        match obj {
            PdfObject::Integer(n) => {
                current_code = u8::try_from(*n).ok();
            }
            PdfObject::Name(name) => {
                if let Some(code) = current_code {
                    result.push((code, name.clone()));
                    current_code = code.checked_add(1);
                }
            }
            _ => {}
        }
    }
    result
}

/// The Unicode text of a glyph name, by the Adobe Glyph List rules: the part
/// before the first `.`, split at `_`, each component looked up in the AGL or
/// read as `uniXXXX...` / `uXXXX`-`uXXXXXX`. `None` when nothing maps.
pub fn glyph_name_to_unicode(name: &[u8]) -> Option<String> {
    let name = std::str::from_utf8(name).ok()?;
    let base = name.split('.').next().unwrap_or("");
    let text: String = base
        .split('_')
        .filter_map(glyph_component_to_unicode)
        .collect();
    (!text.is_empty()).then_some(text)
}

fn glyph_component_to_unicode(component: &str) -> Option<String> {
    if let Ok(i) = AGL.binary_search_by(|(name, _)| name.as_bytes().cmp(component.as_bytes())) {
        return Some(AGL[i].1.to_string());
    }
    let upper_hex = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
    };
    if let Some(hex) = component.strip_prefix("uni")
        && hex.len() % 4 == 0
        && upper_hex(hex)
    {
        let chars: Option<String> = (0..hex.len())
            .step_by(4)
            .map(|i| {
                u32::from_str_radix(&hex[i..i + 4], 16)
                    .ok()
                    .and_then(char::from_u32)
            })
            .collect();
        if chars.is_some() {
            return chars;
        }
    }
    if let Some(hex) = component.strip_prefix('u')
        && (4..=6).contains(&hex.len())
        && upper_hex(hex)
    {
        return u32::from_str_radix(hex, 16)
            .ok()
            .and_then(char::from_u32)
            .map(String::from);
    }
    None
}

/// PDFDocEncoding decoding.
fn decode_pdfdoc(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| PDFDOC_TO_UNICODE[b as usize])
        .collect()
}

/// Windows-1252 to Unicode mapping table.
static WINANSI_TO_UNICODE: [char; 256] = {
    let mut table = ['\0'; 256];
    let mut i = 0;
    while i < 128 {
        table[i] = i as u8 as char;
        i += 1;
    }
    while i < 256 {
        table[i] = i as u8 as char; // default: Latin-1
        i += 1;
    }
    // Windows-1252 specific mappings (0x80-0x9F)
    table[0x80] = '\u{20AC}'; // Euro sign
    table[0x82] = '\u{201A}'; // Single low-9 quotation mark
    table[0x83] = '\u{0192}'; // Latin small letter f with hook
    table[0x84] = '\u{201E}'; // Double low-9 quotation mark
    table[0x85] = '\u{2026}'; // Horizontal ellipsis
    table[0x86] = '\u{2020}'; // Dagger
    table[0x87] = '\u{2021}'; // Double dagger
    table[0x88] = '\u{02C6}'; // Modifier letter circumflex accent
    table[0x89] = '\u{2030}'; // Per mille sign
    table[0x8A] = '\u{0160}'; // Latin capital letter S with caron
    table[0x8B] = '\u{2039}'; // Single left-pointing angle quotation mark
    table[0x8C] = '\u{0152}'; // Latin capital ligature OE
    table[0x8E] = '\u{017D}'; // Latin capital letter Z with caron
    table[0x91] = '\u{2018}'; // Left single quotation mark
    table[0x92] = '\u{2019}'; // Right single quotation mark
    table[0x93] = '\u{201C}'; // Left double quotation mark
    table[0x94] = '\u{201D}'; // Right double quotation mark
    table[0x95] = '\u{2022}'; // Bullet
    table[0x96] = '\u{2013}'; // En dash
    table[0x97] = '\u{2014}'; // Em dash
    table[0x98] = '\u{02DC}'; // Small tilde
    table[0x99] = '\u{2122}'; // Trade mark sign
    table[0x9A] = '\u{0161}'; // Latin small letter s with caron
    table[0x9B] = '\u{203A}'; // Single right-pointing angle quotation mark
    table[0x9C] = '\u{0153}'; // Latin small ligature oe
    table[0x9E] = '\u{017E}'; // Latin small letter z with caron
    table[0x9F] = '\u{0178}'; // Latin capital letter Y with diaeresis
    table
};

/// PDFDocEncoding to Unicode (identical to WinAnsi for most codes).
static PDFDOC_TO_UNICODE: [char; 256] = {
    let mut table = WINANSI_TO_UNICODE;
    // PDFDocEncoding differences from WinAnsi in 0x80-0x9F and some control chars
    // (Simplified: use WinAnsi as base)
    table[0x7F] = '\u{FFFD}'; // Undefined
    table[0x80] = '\u{2022}'; // Bullet (different from WinAnsi)
    table[0xAD] = '\u{00AD}'; // Soft hyphen
    table
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agl_is_sorted_for_binary_search() {
        assert!(
            AGL.windows(2)
                .all(|w| w[0].0.as_bytes() < w[1].0.as_bytes())
        );
    }

    #[test]
    fn glyph_names_follow_the_agl_rules() {
        assert_eq!(glyph_name_to_unicode(b"eacute").as_deref(), Some("\u{e9}"));
        assert_eq!(glyph_name_to_unicode(b"a.sc").as_deref(), Some("a"));
        assert_eq!(glyph_name_to_unicode(b"f_f_i").as_deref(), Some("ffi"));
        assert_eq!(glyph_name_to_unicode(b"uni00410042").as_deref(), Some("AB"));
        assert_eq!(
            glyph_name_to_unicode(b"u1F600").as_deref(),
            Some("\u{1F600}")
        );
        // lower-case hex is not a uni name
        assert_eq!(glyph_name_to_unicode(b"uni00e9"), None);
        assert_eq!(glyph_name_to_unicode(b"g123"), None);
        // a surrogate is not a character
        assert_eq!(glyph_name_to_unicode(b"uniD800"), None);
    }

    #[test]
    fn test_decode_ascii() {
        let result = decode_text(b"Hello", Encoding::WinAnsiEncoding);
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_decode_utf16be_bom() {
        let data = [0xFE, 0xFF, 0x00, 0x48, 0x00, 0x69]; // "Hi"
        let result = decode_text(&data, Encoding::WinAnsiEncoding);
        assert_eq!(result, "Hi");
    }

    #[test]
    fn test_decode_winansi_special() {
        // Euro sign (0x80 in WinAnsi)
        let result = decode_text(&[0x80], Encoding::WinAnsiEncoding);
        assert_eq!(result, "\u{20AC}");
    }

    #[test]
    fn test_encoding_from_name() {
        assert_eq!(
            Encoding::from_name(b"WinAnsiEncoding"),
            Encoding::WinAnsiEncoding
        );
        assert_eq!(
            Encoding::from_name(b"MacRomanEncoding"),
            Encoding::MacRomanEncoding
        );
        assert_eq!(Encoding::from_name(b"Identity-H"), Encoding::Identity);
    }
}
