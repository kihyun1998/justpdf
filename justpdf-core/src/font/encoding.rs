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
        Encoding::MacRomanEncoding => decode_mac_roman(bytes),
        Encoding::PDFDocEncoding => decode_pdfdoc(bytes),
        Encoding::StandardEncoding => decode_winansi(bytes), // close enough for display
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
                let cp =
                    0x10000 + ((code as u32 - 0xD800) << 10) + (low as u32 - 0xDC00);
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

/// Mac Roman decoding (simplified — uses same table for now).
fn decode_mac_roman(bytes: &[u8]) -> String {
    // Simplified: just use the byte value as a char for ASCII range
    bytes
        .iter()
        .map(|&b| {
            if b < 128 {
                b as char
            } else {
                WINANSI_TO_UNICODE[b as usize] // approximate
            }
        })
        .collect()
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
