mod encoding;
mod standard14;

pub use encoding::{Encoding, decode_text};
pub use standard14::{is_standard14, standard14_widths};

use crate::object::{PdfDict, PdfObject};

/// Basic font information extracted from a PDF font dictionary.
#[derive(Debug, Clone)]
pub struct FontInfo {
    /// Font name (BaseFont).
    pub base_font: Vec<u8>,
    /// Font subtype: Type1, TrueType, Type0, Type3, etc.
    pub subtype: Vec<u8>,
    /// Encoding used by this font.
    pub encoding: Encoding,
    /// Glyph widths (indexed by char code).
    pub widths: FontWidths,
    /// ToUnicode CMap data (raw, not parsed yet).
    pub to_unicode: Option<Vec<u8>>,
    /// Whether this is one of the Standard 14 fonts.
    pub is_standard14: bool,
}

/// Font glyph widths.
#[derive(Debug, Clone)]
pub enum FontWidths {
    /// Simple font: /FirstChar, /LastChar, /Widths array.
    Simple {
        first_char: u32,
        widths: Vec<f64>,
        default_width: f64,
    },
    /// CID font: /W array (not yet fully parsed).
    CID {
        default_width: f64,
        w_entries: Vec<CIDWidthEntry>,
    },
    /// No width info — use default.
    None { default_width: f64 },
}

#[derive(Debug, Clone)]
pub enum CIDWidthEntry {
    /// CID range: first_cid, last_cid, width.
    Range { first: u32, last: u32, width: f64 },
    /// CID list: first_cid, [w1, w2, ...].
    List { first: u32, widths: Vec<f64> },
}

impl FontWidths {
    /// Get width for a character code (in font units, typically 1/1000 of text space).
    pub fn get_width(&self, char_code: u32) -> f64 {
        match self {
            Self::Simple {
                first_char,
                widths,
                default_width,
            } => {
                if char_code >= *first_char {
                    let idx = (char_code - first_char) as usize;
                    widths.get(idx).copied().unwrap_or(*default_width)
                } else {
                    *default_width
                }
            }
            Self::CID {
                default_width,
                w_entries,
            } => {
                for entry in w_entries {
                    match entry {
                        CIDWidthEntry::Range { first, last, width } => {
                            if char_code >= *first && char_code <= *last {
                                return *width;
                            }
                        }
                        CIDWidthEntry::List { first, widths } => {
                            if char_code >= *first {
                                let idx = (char_code - first) as usize;
                                if let Some(w) = widths.get(idx) {
                                    return *w;
                                }
                            }
                        }
                    }
                }
                *default_width
            }
            Self::None { default_width } => *default_width,
        }
    }
}

/// Parse basic font info from a font dictionary.
pub fn parse_font_info(dict: &PdfDict) -> FontInfo {
    let base_font = dict
        .get(b"BaseFont")
        .and_then(|o| o.as_name())
        .unwrap_or(b"Unknown")
        .to_vec();

    let subtype = dict
        .get(b"Subtype")
        .and_then(|o| o.as_name())
        .unwrap_or(b"Type1")
        .to_vec();

    let is_std14 = is_standard14(&base_font);

    let encoding = parse_encoding(dict);
    let widths = parse_widths(dict, &base_font, is_std14);

    FontInfo {
        base_font,
        subtype,
        encoding,
        widths,
        to_unicode: None, // Resolved later by the document
        is_standard14: is_std14,
    }
}

fn parse_encoding(dict: &PdfDict) -> Encoding {
    match dict.get(b"Encoding") {
        Some(PdfObject::Name(name)) => Encoding::from_name(name),
        // TODO: handle encoding dict with /Differences
        _ => Encoding::StandardEncoding,
    }
}

fn parse_widths(dict: &PdfDict, base_font: &[u8], is_std14: bool) -> FontWidths {
    // Simple font widths
    if let (Some(first_char), Some(widths_arr)) =
        (dict.get_i64(b"FirstChar"), dict.get_array(b"Widths"))
    {
        let widths: Vec<f64> = widths_arr
            .iter()
            .map(|o| o.as_f64().unwrap_or(0.0))
            .collect();
        return FontWidths::Simple {
            first_char: first_char as u32,
            widths,
            default_width: if is_std14 { 600.0 } else { 1000.0 },
        };
    }

    // Standard 14: use built-in widths
    if is_std14 {
        let widths = standard14_widths(base_font);
        if !widths.is_empty() {
            return FontWidths::Simple {
                first_char: 0,
                widths,
                default_width: 600.0,
            };
        }
    }

    FontWidths::None {
        default_width: 1000.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_widths() {
        let widths = FontWidths::Simple {
            first_char: 32,
            widths: vec![250.0, 333.0, 408.0],
            default_width: 0.0,
        };
        assert_eq!(widths.get_width(32), 250.0);
        assert_eq!(widths.get_width(33), 333.0);
        assert_eq!(widths.get_width(34), 408.0);
        assert_eq!(widths.get_width(35), 0.0); // default
        assert_eq!(widths.get_width(0), 0.0); // below first_char
    }

    #[test]
    fn test_cid_widths() {
        let widths = FontWidths::CID {
            default_width: 1000.0,
            w_entries: vec![
                CIDWidthEntry::Range {
                    first: 1,
                    last: 10,
                    width: 500.0,
                },
                CIDWidthEntry::List {
                    first: 20,
                    widths: vec![600.0, 700.0, 800.0],
                },
            ],
        };
        assert_eq!(widths.get_width(5), 500.0);
        assert_eq!(widths.get_width(20), 600.0);
        assert_eq!(widths.get_width(21), 700.0);
        assert_eq!(widths.get_width(99), 1000.0); // default
    }

    #[test]
    fn test_parse_font_info_standard14() {
        let mut dict = PdfDict::new();
        dict.insert(b"Type".to_vec(), PdfObject::Name(b"Font".to_vec()));
        dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Type1".to_vec()));
        dict.insert(
            b"BaseFont".to_vec(),
            PdfObject::Name(b"Helvetica".to_vec()),
        );
        dict.insert(
            b"Encoding".to_vec(),
            PdfObject::Name(b"WinAnsiEncoding".to_vec()),
        );

        let info = parse_font_info(&dict);
        assert_eq!(info.base_font, b"Helvetica");
        assert!(info.is_standard14);
        assert_eq!(info.encoding, Encoding::WinAnsiEncoding);
    }
}
