use crate::object::{IndirectRef, PdfDict, PdfObject};

/// Default Type3 font matrix (maps glyph space to text space).
pub const DEFAULT_FONT_MATRIX: [f64; 6] = [0.001, 0.0, 0.0, 0.001, 0.0, 0.0];

/// Parsed Type3 font data.
///
/// Type3 fonts define each glyph as a PDF content stream (like a mini-page).
/// The font dictionary contains `/CharProcs` (a dict mapping glyph names to
/// content streams) and `/FontMatrix` (transforms glyph space to text space).
#[derive(Debug, Clone)]
pub struct Type3Font {
    /// Font matrix (transforms glyph space to text space).
    /// Default is `[0.001 0 0 0.001 0 0]`.
    pub font_matrix: [f64; 6],
    /// Font bounding box `[llx lly urx ury]`.
    pub font_bbox: [f64; 4],
    /// Character procedures: maps glyph name to content stream reference.
    pub char_procs: Vec<(Vec<u8>, IndirectRef)>,
    /// Encoding: maps character code to glyph name.
    pub encoding: Vec<(u8, Vec<u8>)>,
    /// First character code defined.
    pub first_char: u8,
    /// Last character code defined.
    pub last_char: u8,
    /// Widths for each character code from `first_char` to `last_char`.
    pub widths: Vec<f64>,
    /// Resources dictionary for executing glyph streams.
    pub resources: Option<PdfObject>,
}

/// Parse a Type3 font dictionary into a `Type3Font`.
///
/// Returns `None` if the dictionary lacks required entries (`/CharProcs`, `/FontBBox`).
pub fn parse_type3_font(dict: &PdfDict) -> Option<Type3Font> {
    // /FontBBox — required
    let font_bbox = parse_bbox(dict)?;

    // /FontMatrix — optional, default [0.001 0 0 0.001 0 0]
    let font_matrix = parse_font_matrix(dict);

    // /CharProcs — required
    let char_procs = parse_char_procs(dict)?;

    // /Encoding — parse /Differences
    let encoding = parse_encoding_differences(dict);

    // /FirstChar, /LastChar, /Widths
    let first_char = dict.get_i64(b"FirstChar").unwrap_or(0) as u8;
    let last_char = dict.get_i64(b"LastChar").unwrap_or(0) as u8;
    let widths = dict
        .get_array(b"Widths")
        .map(|arr| arr.iter().map(|o| o.as_f64().unwrap_or(0.0)).collect())
        .unwrap_or_default();

    // /Resources — optional
    let resources = dict.get(b"Resources").cloned();

    Some(Type3Font {
        font_matrix,
        font_bbox,
        char_procs,
        encoding,
        first_char,
        last_char,
        widths,
        resources,
    })
}

/// Parse `/FontMatrix` from the dictionary, returning the default if absent.
fn parse_font_matrix(dict: &PdfDict) -> [f64; 6] {
    match dict.get_array(b"FontMatrix") {
        Some(arr) if arr.len() == 6 => [
            arr[0].as_f64().unwrap_or(0.0),
            arr[1].as_f64().unwrap_or(0.0),
            arr[2].as_f64().unwrap_or(0.0),
            arr[3].as_f64().unwrap_or(0.0),
            arr[4].as_f64().unwrap_or(0.0),
            arr[5].as_f64().unwrap_or(0.0),
        ],
        _ => DEFAULT_FONT_MATRIX,
    }
}

/// Parse `/FontBBox` from the dictionary.
fn parse_bbox(dict: &PdfDict) -> Option<[f64; 4]> {
    let arr = dict.get_array(b"FontBBox")?;
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
}

/// Parse `/CharProcs` dictionary, extracting glyph name to stream reference mappings.
fn parse_char_procs(dict: &PdfDict) -> Option<Vec<(Vec<u8>, IndirectRef)>> {
    let procs_dict = dict.get_dict(b"CharProcs")?;
    let mut result = Vec::new();
    for (name, obj) in procs_dict.iter() {
        if let PdfObject::Reference(r) = obj {
            result.push((name.clone(), r.clone()));
        }
    }
    Some(result)
}

/// Parse encoding `/Differences` array from an `/Encoding` dictionary.
///
/// The `/Differences` array has the form: `[code name1 name2 ... code name1 ...]`
/// where each integer sets the current code, and subsequent names are assigned
/// incrementing codes.
fn parse_encoding_differences(dict: &PdfDict) -> Vec<(u8, Vec<u8>)> {
    let enc_dict = match dict.get(b"Encoding") {
        Some(PdfObject::Dict(d)) => d,
        _ => return Vec::new(),
    };

    let differences = match enc_dict.get_array(b"Differences") {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    let mut result = Vec::new();
    let mut current_code: Option<u8> = None;

    for obj in differences {
        match obj {
            PdfObject::Integer(n) => {
                current_code = Some(*n as u8);
            }
            PdfObject::Name(name) => {
                if let Some(code) = current_code {
                    result.push((code, name.clone()));
                    current_code = Some(code.wrapping_add(1));
                }
            }
            _ => {}
        }
    }

    result
}

impl Type3Font {
    /// Get the width for a given character code.
    pub fn get_width(&self, char_code: u8) -> f64 {
        if char_code >= self.first_char && char_code <= self.last_char {
            let idx = (char_code - self.first_char) as usize;
            self.widths.get(idx).copied().unwrap_or(0.0)
        } else {
            0.0
        }
    }

    /// Look up the glyph name for a character code using the encoding.
    pub fn glyph_name(&self, char_code: u8) -> Option<&[u8]> {
        self.encoding
            .iter()
            .find(|(code, _)| *code == char_code)
            .map(|(_, name)| name.as_slice())
    }

    /// Look up the content stream reference for a glyph by name.
    pub fn char_proc(&self, glyph_name: &[u8]) -> Option<&IndirectRef> {
        self.char_procs
            .iter()
            .find(|(name, _)| name == glyph_name)
            .map(|(_, r)| r)
    }

    /// Resolve a character code to its content stream reference.
    ///
    /// First looks up the glyph name from the encoding, then finds
    /// the corresponding entry in `/CharProcs`.
    pub fn resolve_char_proc(&self, char_code: u8) -> Option<&IndirectRef> {
        let name = self.glyph_name(char_code)?;
        self.char_proc(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a minimal valid Type3 font dictionary.
    fn make_type3_dict() -> PdfDict {
        let mut dict = PdfDict::new();
        dict.insert(b"Type".to_vec(), PdfObject::Name(b"Font".to_vec()));
        dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Type3".to_vec()));

        // FontBBox
        dict.insert(
            b"FontBBox".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Integer(0),
                PdfObject::Integer(0),
                PdfObject::Integer(1000),
                PdfObject::Integer(1000),
            ]),
        );

        // FontMatrix
        dict.insert(
            b"FontMatrix".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Real(0.001),
                PdfObject::Integer(0),
                PdfObject::Integer(0),
                PdfObject::Real(0.001),
                PdfObject::Integer(0),
                PdfObject::Integer(0),
            ]),
        );

        // CharProcs
        let mut char_procs = PdfDict::new();
        char_procs.insert(
            b"a".to_vec(),
            PdfObject::Reference(IndirectRef {
                obj_num: 10,
                gen_num: 0,
            }),
        );
        char_procs.insert(
            b"b".to_vec(),
            PdfObject::Reference(IndirectRef {
                obj_num: 11,
                gen_num: 0,
            }),
        );
        char_procs.insert(
            b"space".to_vec(),
            PdfObject::Reference(IndirectRef {
                obj_num: 12,
                gen_num: 0,
            }),
        );
        dict.insert(b"CharProcs".to_vec(), PdfObject::Dict(char_procs));

        // Encoding with Differences
        let mut enc = PdfDict::new();
        enc.insert(
            b"Differences".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Integer(32),
                PdfObject::Name(b"space".to_vec()),
                PdfObject::Integer(97),
                PdfObject::Name(b"a".to_vec()),
                PdfObject::Name(b"b".to_vec()),
            ]),
        );
        dict.insert(b"Encoding".to_vec(), PdfObject::Dict(enc));

        // FirstChar, LastChar, Widths
        dict.insert(b"FirstChar".to_vec(), PdfObject::Integer(32));
        dict.insert(b"LastChar".to_vec(), PdfObject::Integer(98));
        let mut widths = vec![PdfObject::Integer(0); 67]; // 32..98 = 67 entries
        widths[0] = PdfObject::Integer(250); // space (32)
        widths[65] = PdfObject::Integer(500); // a (97)
        widths[66] = PdfObject::Integer(600); // b (98)
        dict.insert(b"Widths".to_vec(), PdfObject::Array(widths));

        dict
    }

    #[test]
    fn test_parse_type3_font() {
        let dict = make_type3_dict();
        let font = parse_type3_font(&dict).expect("should parse Type3 font");

        assert_eq!(font.font_matrix, [0.001, 0.0, 0.0, 0.001, 0.0, 0.0]);
        assert_eq!(font.font_bbox, [0.0, 0.0, 1000.0, 1000.0]);
        assert_eq!(font.first_char, 32);
        assert_eq!(font.last_char, 98);
        assert_eq!(font.widths.len(), 67);
        assert!(!font.char_procs.is_empty());
    }

    #[test]
    fn test_font_matrix_default() {
        let mut dict = make_type3_dict();
        dict.remove(b"FontMatrix");

        let font = parse_type3_font(&dict).expect("should parse");
        assert_eq!(font.font_matrix, DEFAULT_FONT_MATRIX);
    }

    #[test]
    fn test_encoding_differences_parsing() {
        let dict = make_type3_dict();
        let font = parse_type3_font(&dict).expect("should parse");

        // Differences: [32 /space 97 /a /b]
        // => (32, "space"), (97, "a"), (98, "b")
        assert_eq!(font.encoding.len(), 3);
        assert_eq!(font.encoding[0], (32, b"space".to_vec()));
        assert_eq!(font.encoding[1], (97, b"a".to_vec()));
        assert_eq!(font.encoding[2], (98, b"b".to_vec()));
    }

    #[test]
    fn test_missing_char_procs() {
        let mut dict = PdfDict::new();
        dict.insert(
            b"FontBBox".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Integer(0),
                PdfObject::Integer(0),
                PdfObject::Integer(1000),
                PdfObject::Integer(1000),
            ]),
        );
        // No CharProcs => should return None
        assert!(parse_type3_font(&dict).is_none());
    }

    #[test]
    fn test_width_extraction() {
        let dict = make_type3_dict();
        let font = parse_type3_font(&dict).expect("should parse");

        assert_eq!(font.get_width(32), 250.0); // space
        assert_eq!(font.get_width(97), 500.0); // a
        assert_eq!(font.get_width(98), 600.0); // b
        assert_eq!(font.get_width(50), 0.0); // some code with zero width
        assert_eq!(font.get_width(0), 0.0); // below first_char
        assert_eq!(font.get_width(255), 0.0); // above last_char
    }

    #[test]
    fn test_glyph_name_lookup() {
        let dict = make_type3_dict();
        let font = parse_type3_font(&dict).expect("should parse");

        assert_eq!(font.glyph_name(32), Some(b"space".as_slice()));
        assert_eq!(font.glyph_name(97), Some(b"a".as_slice()));
        assert_eq!(font.glyph_name(98), Some(b"b".as_slice()));
        assert_eq!(font.glyph_name(0), None);
    }

    #[test]
    fn test_char_proc_lookup() {
        let dict = make_type3_dict();
        let font = parse_type3_font(&dict).expect("should parse");

        let a_ref = font.char_proc(b"a").expect("should find 'a'");
        assert_eq!(a_ref.obj_num, 10);

        let space_ref = font.char_proc(b"space").expect("should find 'space'");
        assert_eq!(space_ref.obj_num, 12);

        assert!(font.char_proc(b"z").is_none());
    }

    #[test]
    fn test_resolve_char_proc() {
        let dict = make_type3_dict();
        let font = parse_type3_font(&dict).expect("should parse");

        let r = font.resolve_char_proc(97).expect("should resolve 'a'");
        assert_eq!(r.obj_num, 10);

        let r = font.resolve_char_proc(32).expect("should resolve space");
        assert_eq!(r.obj_num, 12);

        assert!(font.resolve_char_proc(0).is_none());
    }

    #[test]
    fn test_missing_font_bbox() {
        let mut dict = PdfDict::new();
        let mut char_procs = PdfDict::new();
        char_procs.insert(
            b"a".to_vec(),
            PdfObject::Reference(IndirectRef {
                obj_num: 1,
                gen_num: 0,
            }),
        );
        dict.insert(b"CharProcs".to_vec(), PdfObject::Dict(char_procs));
        // No FontBBox => should return None
        assert!(parse_type3_font(&dict).is_none());
    }

    #[test]
    fn test_resources_present() {
        let mut dict = make_type3_dict();
        let mut res = PdfDict::new();
        res.insert(
            b"Font".to_vec(),
            PdfObject::Reference(IndirectRef {
                obj_num: 99,
                gen_num: 0,
            }),
        );
        dict.insert(b"Resources".to_vec(), PdfObject::Dict(res));

        let font = parse_type3_font(&dict).expect("should parse");
        assert!(font.resources.is_some());
    }

    #[test]
    fn test_empty_encoding() {
        let mut dict = make_type3_dict();
        dict.remove(b"Encoding");

        let font = parse_type3_font(&dict).expect("should parse");
        assert!(font.encoding.is_empty());
    }
}
