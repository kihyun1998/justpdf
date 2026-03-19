pub mod cff;
pub mod cjk;
pub mod cmap;
mod encoding;
pub mod opentype;
pub mod recovery;
mod standard14;
pub mod subset;
pub mod type3;

pub use cmap::ToUnicodeCMap;
pub use encoding::{Encoding, decode_text};
pub use standard14::{is_standard14, standard14_widths};

use crate::object::{IndirectRef, PdfDict, PdfObject};

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
    /// Font descriptor (PDF spec 7.6).
    pub descriptor: Option<FontDescriptor>,
}

/// Font descriptor containing font metrics and flags (PDF spec section 7.6).
#[derive(Debug, Clone)]
pub struct FontDescriptor {
    /// /FontName — the PostScript name of the font.
    pub font_name: Vec<u8>,
    /// /FontFamily — the font family name.
    pub font_family: Option<Vec<u8>>,
    /// /Flags — a collection of boolean attributes (see flag constants).
    pub flags: u32,
    /// /FontBBox — bounding box for all glyphs [llx lly urx ury].
    pub font_b_box: Option<[f64; 4]>,
    /// /ItalicAngle — angle in degrees counter-clockwise from vertical.
    pub italic_angle: f64,
    /// /Ascent — maximum height above the baseline.
    pub ascent: f64,
    /// /Descent — maximum depth below the baseline (typically negative).
    pub descent: f64,
    /// /CapHeight — top of flat capital letters.
    pub cap_height: Option<f64>,
    /// /XHeight — top of flat non-ascending lowercase letters.
    pub x_height: Option<f64>,
    /// /StemV — dominant vertical stem width.
    pub stem_v: f64,
    /// /StemH — dominant horizontal stem width.
    pub stem_h: Option<f64>,
    /// /AvgWidth — average glyph width.
    pub avg_width: Option<f64>,
    /// /MaxWidth — maximum glyph width.
    pub max_width: Option<f64>,
    /// /MissingWidth — width to use for undefined character codes.
    pub missing_width: Option<f64>,
    /// /Leading — desired spacing between lines of text.
    pub leading: Option<f64>,
    /// /FontFile — reference to embedded Type1 font program.
    pub font_file_ref: Option<IndirectRef>,
    /// /FontFile2 — reference to embedded TrueType font program.
    pub font_file2_ref: Option<IndirectRef>,
    /// /FontFile3 — reference to embedded CFF / OpenType font program.
    pub font_file3_ref: Option<IndirectRef>,
}

impl FontDescriptor {
    // Font flag constants (PDF spec Table 123).
    /// All glyphs have the same width.
    pub const FIXED_PITCH: u32 = 1;
    /// Glyphs have serifs.
    pub const SERIF: u32 = 1 << 1;
    /// Font uses a symbol character set.
    pub const SYMBOLIC: u32 = 1 << 2;
    /// Glyphs resemble cursive handwriting.
    pub const SCRIPT: u32 = 1 << 3;
    /// Font uses a standard (non-symbol) character set.
    pub const NONSYMBOLIC: u32 = 1 << 5;
    /// Glyphs have dominant slant.
    pub const ITALIC: u32 = 1 << 6;
    /// No lowercase letters; small letters rendered as small capitals.
    pub const ALL_CAP: u32 = 1 << 16;
    /// Lowercase letters rendered as smaller versions of uppercase.
    pub const SMALL_CAP: u32 = 1 << 17;
    /// Bold glyphs shall be painted with extra thickness at small sizes.
    pub const FORCE_BOLD: u32 = 1 << 18;

    /// Check whether the given flag bit is set.
    pub fn has_flag(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }

    /// Returns true if the font is fixed-pitch.
    pub fn is_fixed_pitch(&self) -> bool {
        self.has_flag(Self::FIXED_PITCH)
    }

    /// Returns true if the font is symbolic.
    pub fn is_symbolic(&self) -> bool {
        self.has_flag(Self::SYMBOLIC)
    }

    /// Returns true if the font is italic.
    pub fn is_italic(&self) -> bool {
        self.has_flag(Self::ITALIC)
    }
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

    let descriptor = dict
        .get_dict(b"FontDescriptor")
        .and_then(parse_font_descriptor);

    FontInfo {
        base_font,
        subtype,
        encoding,
        widths,
        to_unicode: None, // Resolved later by the document
        is_standard14: is_std14,
        descriptor,
    }
}

/// Parse a font descriptor dictionary (PDF spec section 7.6).
///
/// Returns `None` if the dictionary lacks the required `/FontName` entry.
pub fn parse_font_descriptor(dict: &PdfDict) -> Option<FontDescriptor> {
    let font_name = dict
        .get(b"FontName")
        .and_then(|o| o.as_name())
        .map(|n| n.to_vec())?;

    let font_family = dict
        .get(b"FontFamily")
        .and_then(|o| match o {
            PdfObject::String(s) => Some(s.clone()),
            _ => o.as_name().map(|n| n.to_vec()),
        });

    let flags = dict.get_i64(b"Flags").unwrap_or(0) as u32;
    let italic_angle = dict.get_f64(b"ItalicAngle").unwrap_or(0.0);
    let ascent = dict.get_f64(b"Ascent").unwrap_or(0.0);
    let descent = dict.get_f64(b"Descent").unwrap_or(0.0);
    let stem_v = dict.get_f64(b"StemV").unwrap_or(0.0);

    let font_b_box = dict.get_array(b"FontBBox").and_then(|arr| {
        if arr.len() == 4 {
            Some([
                arr[0].as_f64().unwrap_or(0.0),
                arr[1].as_f64().unwrap_or(0.0),
                arr[2].as_f64().unwrap_or(0.0),
                arr[3].as_f64().unwrap_or(0.0),
            ])
        } else {
            None
        }
    });

    let cap_height = dict.get_f64(b"CapHeight");
    let x_height = dict.get_f64(b"XHeight");
    let stem_h = dict.get_f64(b"StemH");
    let avg_width = dict.get_f64(b"AvgWidth");
    let max_width = dict.get_f64(b"MaxWidth");
    let missing_width = dict.get_f64(b"MissingWidth");
    let leading = dict.get_f64(b"Leading");

    let font_file_ref = dict.get_ref(b"FontFile").cloned();
    let font_file2_ref = dict.get_ref(b"FontFile2").cloned();
    let font_file3_ref = dict.get_ref(b"FontFile3").cloned();

    Some(FontDescriptor {
        font_name,
        font_family,
        flags,
        font_b_box,
        italic_angle,
        ascent,
        descent,
        cap_height,
        x_height,
        stem_v,
        stem_h,
        avg_width,
        max_width,
        missing_width,
        leading,
        font_file_ref,
        font_file2_ref,
        font_file3_ref,
    })
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
        dict.insert(b"BaseFont".to_vec(), PdfObject::Name(b"Helvetica".to_vec()));
        dict.insert(
            b"Encoding".to_vec(),
            PdfObject::Name(b"WinAnsiEncoding".to_vec()),
        );

        let info = parse_font_info(&dict);
        assert_eq!(info.base_font, b"Helvetica");
        assert!(info.is_standard14);
        assert_eq!(info.encoding, Encoding::WinAnsiEncoding);
        assert!(info.descriptor.is_none());
    }

    #[test]
    fn test_parse_font_descriptor_full() {
        let mut desc = PdfDict::new();
        desc.insert(b"FontName".to_vec(), PdfObject::Name(b"ArialMT".to_vec()));
        desc.insert(
            b"FontFamily".to_vec(),
            PdfObject::String(b"Arial".to_vec()),
        );
        desc.insert(b"Flags".to_vec(), PdfObject::Integer(32)); // NONSYMBOLIC
        desc.insert(
            b"FontBBox".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Integer(-665),
                PdfObject::Integer(-210),
                PdfObject::Integer(2000),
                PdfObject::Integer(728),
            ]),
        );
        desc.insert(b"ItalicAngle".to_vec(), PdfObject::Integer(0));
        desc.insert(b"Ascent".to_vec(), PdfObject::Integer(905));
        desc.insert(b"Descent".to_vec(), PdfObject::Integer(-212));
        desc.insert(b"CapHeight".to_vec(), PdfObject::Integer(728));
        desc.insert(b"XHeight".to_vec(), PdfObject::Integer(517));
        desc.insert(b"StemV".to_vec(), PdfObject::Integer(88));
        desc.insert(b"StemH".to_vec(), PdfObject::Integer(76));
        desc.insert(b"AvgWidth".to_vec(), PdfObject::Integer(441));
        desc.insert(b"MaxWidth".to_vec(), PdfObject::Integer(2000));
        desc.insert(b"MissingWidth".to_vec(), PdfObject::Integer(250));
        desc.insert(b"Leading".to_vec(), PdfObject::Integer(33));
        desc.insert(
            b"FontFile2".to_vec(),
            PdfObject::Reference(IndirectRef {
                obj_num: 42,
                gen_num: 0,
            }),
        );

        let fd = parse_font_descriptor(&desc).expect("should parse");
        assert_eq!(fd.font_name, b"ArialMT");
        assert_eq!(fd.font_family.as_deref(), Some(b"Arial".as_slice()));
        assert_eq!(fd.flags, 32);
        assert!(fd.has_flag(FontDescriptor::NONSYMBOLIC));
        assert!(!fd.has_flag(FontDescriptor::SYMBOLIC));
        assert!(!fd.is_italic());
        assert!(!fd.is_fixed_pitch());
        assert!(!fd.is_symbolic());

        let bbox = fd.font_b_box.expect("should have bbox");
        assert_eq!(bbox, [-665.0, -210.0, 2000.0, 728.0]);

        assert_eq!(fd.italic_angle, 0.0);
        assert_eq!(fd.ascent, 905.0);
        assert_eq!(fd.descent, -212.0);
        assert_eq!(fd.cap_height, Some(728.0));
        assert_eq!(fd.x_height, Some(517.0));
        assert_eq!(fd.stem_v, 88.0);
        assert_eq!(fd.stem_h, Some(76.0));
        assert_eq!(fd.avg_width, Some(441.0));
        assert_eq!(fd.max_width, Some(2000.0));
        assert_eq!(fd.missing_width, Some(250.0));
        assert_eq!(fd.leading, Some(33.0));

        assert!(fd.font_file_ref.is_none());
        let ff2 = fd.font_file2_ref.as_ref().expect("should have FontFile2");
        assert_eq!(ff2.obj_num, 42);
        assert_eq!(ff2.gen_num, 0);
        assert!(fd.font_file3_ref.is_none());
    }

    #[test]
    fn test_parse_font_descriptor_minimal() {
        let mut desc = PdfDict::new();
        desc.insert(b"FontName".to_vec(), PdfObject::Name(b"MyFont".to_vec()));

        let fd = parse_font_descriptor(&desc).expect("should parse minimal");
        assert_eq!(fd.font_name, b"MyFont");
        assert_eq!(fd.flags, 0);
        assert_eq!(fd.italic_angle, 0.0);
        assert_eq!(fd.ascent, 0.0);
        assert_eq!(fd.descent, 0.0);
        assert_eq!(fd.stem_v, 0.0);
        assert!(fd.font_family.is_none());
        assert!(fd.font_b_box.is_none());
        assert!(fd.cap_height.is_none());
        assert!(fd.x_height.is_none());
        assert!(fd.stem_h.is_none());
        assert!(fd.avg_width.is_none());
        assert!(fd.max_width.is_none());
        assert!(fd.missing_width.is_none());
        assert!(fd.leading.is_none());
        assert!(fd.font_file_ref.is_none());
        assert!(fd.font_file2_ref.is_none());
        assert!(fd.font_file3_ref.is_none());
    }

    #[test]
    fn test_parse_font_descriptor_missing_font_name() {
        let mut desc = PdfDict::new();
        desc.insert(b"Flags".to_vec(), PdfObject::Integer(32));
        desc.insert(b"Ascent".to_vec(), PdfObject::Integer(800));

        assert!(parse_font_descriptor(&desc).is_none());
    }

    #[test]
    fn test_font_descriptor_flags() {
        let fd = FontDescriptor {
            font_name: b"TestFont".to_vec(),
            font_family: None,
            flags: FontDescriptor::FIXED_PITCH
                | FontDescriptor::SERIF
                | FontDescriptor::ITALIC
                | FontDescriptor::FORCE_BOLD,
            font_b_box: None,
            italic_angle: -12.0,
            ascent: 800.0,
            descent: -200.0,
            cap_height: None,
            x_height: None,
            stem_v: 80.0,
            stem_h: None,
            avg_width: None,
            max_width: None,
            missing_width: None,
            leading: None,
            font_file_ref: None,
            font_file2_ref: None,
            font_file3_ref: None,
        };

        assert!(fd.is_fixed_pitch());
        assert!(fd.has_flag(FontDescriptor::SERIF));
        assert!(fd.is_italic());
        assert!(fd.has_flag(FontDescriptor::FORCE_BOLD));
        assert!(!fd.is_symbolic());
        assert!(!fd.has_flag(FontDescriptor::NONSYMBOLIC));
        assert!(!fd.has_flag(FontDescriptor::SCRIPT));
        assert!(!fd.has_flag(FontDescriptor::ALL_CAP));
        assert!(!fd.has_flag(FontDescriptor::SMALL_CAP));
    }

    #[test]
    fn test_parse_font_descriptor_bbox_wrong_length() {
        let mut desc = PdfDict::new();
        desc.insert(b"FontName".to_vec(), PdfObject::Name(b"Test".to_vec()));
        desc.insert(
            b"FontBBox".to_vec(),
            PdfObject::Array(vec![PdfObject::Integer(0), PdfObject::Integer(0)]),
        );

        let fd = parse_font_descriptor(&desc).expect("should parse");
        assert!(fd.font_b_box.is_none());
    }

    #[test]
    fn test_parse_font_info_with_descriptor() {
        let mut desc_dict = PdfDict::new();
        desc_dict.insert(
            b"FontName".to_vec(),
            PdfObject::Name(b"TimesNewRomanPSMT".to_vec()),
        );
        desc_dict.insert(b"Flags".to_vec(), PdfObject::Integer(34)); // SERIF | NONSYMBOLIC
        desc_dict.insert(b"Ascent".to_vec(), PdfObject::Integer(891));
        desc_dict.insert(b"Descent".to_vec(), PdfObject::Integer(-216));
        desc_dict.insert(b"StemV".to_vec(), PdfObject::Integer(82));
        desc_dict.insert(b"ItalicAngle".to_vec(), PdfObject::Integer(0));
        desc_dict.insert(b"CapHeight".to_vec(), PdfObject::Integer(662));
        desc_dict.insert(
            b"FontFile2".to_vec(),
            PdfObject::Reference(IndirectRef {
                obj_num: 100,
                gen_num: 0,
            }),
        );

        let mut font_dict = PdfDict::new();
        font_dict.insert(b"Type".to_vec(), PdfObject::Name(b"Font".to_vec()));
        font_dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"TrueType".to_vec()));
        font_dict.insert(
            b"BaseFont".to_vec(),
            PdfObject::Name(b"TimesNewRomanPSMT".to_vec()),
        );
        font_dict.insert(
            b"Encoding".to_vec(),
            PdfObject::Name(b"WinAnsiEncoding".to_vec()),
        );
        font_dict.insert(
            b"FontDescriptor".to_vec(),
            PdfObject::Dict(desc_dict),
        );

        let info = parse_font_info(&font_dict);
        assert_eq!(info.base_font, b"TimesNewRomanPSMT");
        assert_eq!(info.subtype, b"TrueType");

        let fd = info.descriptor.expect("should have descriptor");
        assert_eq!(fd.font_name, b"TimesNewRomanPSMT");
        assert_eq!(fd.flags, 34);
        assert!(fd.has_flag(FontDescriptor::SERIF));
        assert!(fd.has_flag(FontDescriptor::NONSYMBOLIC));
        assert_eq!(fd.ascent, 891.0);
        assert_eq!(fd.descent, -216.0);
        assert_eq!(fd.stem_v, 82.0);
        assert_eq!(fd.cap_height, Some(662.0));
        let ff2 = fd.font_file2_ref.as_ref().expect("should have FontFile2");
        assert_eq!(ff2.obj_num, 100);
    }

    #[test]
    fn test_font_descriptor_all_font_file_refs() {
        let mut desc = PdfDict::new();
        desc.insert(b"FontName".to_vec(), PdfObject::Name(b"Test".to_vec()));
        desc.insert(
            b"FontFile".to_vec(),
            PdfObject::Reference(IndirectRef { obj_num: 10, gen_num: 0 }),
        );
        desc.insert(
            b"FontFile2".to_vec(),
            PdfObject::Reference(IndirectRef { obj_num: 20, gen_num: 0 }),
        );
        desc.insert(
            b"FontFile3".to_vec(),
            PdfObject::Reference(IndirectRef { obj_num: 30, gen_num: 0 }),
        );

        let fd = parse_font_descriptor(&desc).expect("should parse");
        assert_eq!(fd.font_file_ref.as_ref().unwrap().obj_num, 10);
        assert_eq!(fd.font_file2_ref.as_ref().unwrap().obj_num, 20);
        assert_eq!(fd.font_file3_ref.as_ref().unwrap().obj_num, 30);
    }
}
