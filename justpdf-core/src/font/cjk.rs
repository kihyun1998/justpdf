//! CJK (Chinese, Japanese, Korean) text writing support using CID fonts.
//!
//! This module provides the data structures and functions needed to create
//! Type0 (composite) fonts for embedding CJK text in PDF documents.

use std::fmt::Write as FmtWrite;

use crate::object::{IndirectRef, PdfDict, PdfObject};
use crate::writer::PdfWriter;
use crate::writer::encode::make_stream;

/// CID font system info (Registry-Ordering-Supplement).
#[derive(Debug, Clone)]
pub struct CIDSystemInfo {
    pub registry: String,
    pub ordering: String,
    pub supplement: i64,
}

impl CIDSystemInfo {
    /// Convert to a PDF dictionary.
    pub fn to_pdf_dict(&self) -> PdfDict {
        let mut dict = PdfDict::new();
        dict.insert(
            b"Registry".to_vec(),
            PdfObject::String(self.registry.as_bytes().to_vec()),
        );
        dict.insert(
            b"Ordering".to_vec(),
            PdfObject::String(self.ordering.as_bytes().to_vec()),
        );
        dict.insert(b"Supplement".to_vec(), PdfObject::Integer(self.supplement));
        dict
    }
}

/// Known CJK orderings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CJKOrdering {
    /// Adobe-Japan1 (Japanese)
    Japan1,
    /// Adobe-GB1 (Simplified Chinese)
    GB1,
    /// Adobe-CNS1 (Traditional Chinese)
    CNS1,
    /// Adobe-Korea1 (Korean)
    Korea1,
    /// Adobe-Identity (generic)
    Identity,
}

impl CJKOrdering {
    /// Get the CIDSystemInfo for this ordering.
    pub fn system_info(&self) -> CIDSystemInfo {
        match self {
            Self::Japan1 => CIDSystemInfo {
                registry: "Adobe".into(),
                ordering: "Japan1".into(),
                supplement: 7,
            },
            Self::GB1 => CIDSystemInfo {
                registry: "Adobe".into(),
                ordering: "GB1".into(),
                supplement: 5,
            },
            Self::CNS1 => CIDSystemInfo {
                registry: "Adobe".into(),
                ordering: "CNS1".into(),
                supplement: 7,
            },
            Self::Korea1 => CIDSystemInfo {
                registry: "Adobe".into(),
                ordering: "Korea1".into(),
                supplement: 9,
            },
            Self::Identity => CIDSystemInfo {
                registry: "Adobe".into(),
                ordering: "Identity".into(),
                supplement: 0,
            },
        }
    }

    /// Get the predefined CMap name for this ordering.
    pub fn cmap_name(&self) -> &'static str {
        match self {
            Self::Japan1 => "UniJIS-UTF16-H",
            Self::GB1 => "UniGB-UTF16-H",
            Self::CNS1 => "UniCNS-UTF16-H",
            Self::Korea1 => "UniKS-UTF16-H",
            Self::Identity => "Identity-H",
        }
    }
}

/// Detect the appropriate CJK ordering from Unicode text.
///
/// Examines the character ranges present in the text:
/// - Japanese: Hiragana (U+3040-309F), Katakana (U+30A0-30FF)
/// - Korean: Hangul Syllables (U+AC00-D7AF), Hangul Jamo (U+1100-11FF)
/// - Simplified Chinese / Traditional Chinese: detected by CJK Unified
///   Ideographs range (U+4E00-9FFF) with disambiguation heuristics
///
/// Returns `CJKOrdering::Identity` for mixed or unrecognized text.
pub fn detect_ordering(text: &str) -> CJKOrdering {
    let mut has_hiragana = false;
    let mut has_katakana = false;
    let mut has_hangul = false;
    let mut has_cjk_unified = false;
    let mut has_bopomofo = false;

    for ch in text.chars() {
        match ch as u32 {
            // Hiragana
            0x3040..=0x309F => has_hiragana = true,
            // Katakana
            0x30A0..=0x30FF => has_katakana = true,
            // Hangul Jamo
            0x1100..=0x11FF => has_hangul = true,
            // Hangul Compatibility Jamo
            0x3130..=0x318F => has_hangul = true,
            // Hangul Syllables
            0xAC00..=0xD7AF => has_hangul = true,
            // Bopomofo (Traditional Chinese phonetic)
            0x3100..=0x312F | 0x31A0..=0x31BF => has_bopomofo = true,
            // CJK Unified Ideographs
            0x4E00..=0x9FFF => has_cjk_unified = true,
            // CJK Extension A
            0x3400..=0x4DBF => has_cjk_unified = true,
            _ => {}
        }
    }

    // Japanese: presence of Hiragana or Katakana is a strong indicator.
    if has_hiragana || has_katakana {
        return CJKOrdering::Japan1;
    }

    // Korean: presence of Hangul.
    if has_hangul {
        return CJKOrdering::Korea1;
    }

    // Traditional Chinese: Bopomofo is a strong indicator.
    if has_bopomofo {
        return CJKOrdering::CNS1;
    }

    // If we have CJK ideographs but no script-specific indicators,
    // default to Simplified Chinese (GB1) as the most common case.
    if has_cjk_unified {
        return CJKOrdering::GB1;
    }

    // No CJK characters detected, use Identity.
    CJKOrdering::Identity
}

/// Generate a ToUnicode CMap for a set of (CID, Unicode) mappings.
///
/// The returned bytes can be used as a PDF stream for the `/ToUnicode` entry
/// of a Type0 font dictionary.
pub fn generate_to_unicode_cmap(mappings: &[(u16, char)]) -> Vec<u8> {
    let mut cmap = String::with_capacity(512 + mappings.len() * 32);

    cmap.push_str("/CIDInit /ProcSet findresource begin\n");
    cmap.push_str("12 dict begin\n");
    cmap.push_str("begincmap\n");
    cmap.push_str("/CIDSystemInfo\n");
    cmap.push_str("<< /Registry (Adobe)\n");
    cmap.push_str("/Ordering (UCS)\n");
    cmap.push_str("/Supplement 0\n");
    cmap.push_str(">> def\n");
    cmap.push_str("/CMapName /Adobe-Identity-UCS def\n");
    cmap.push_str("/CMapType 2 def\n");
    cmap.push_str("1 begincodespacerange\n");
    cmap.push_str("<0000> <FFFF>\n");
    cmap.push_str("endcodespacerange\n");

    if !mappings.is_empty() {
        // Process in chunks of 100 (PDF spec limit for beginbfchar sections).
        for chunk in mappings.chunks(100) {
            let _ = writeln!(cmap, "{} beginbfchar", chunk.len());
            for &(cid, unicode_char) in chunk {
                let unicode_val = unicode_char as u32;
                if unicode_val <= 0xFFFF {
                    let _ = writeln!(cmap, "<{:04X}> <{:04X}>", cid, unicode_val);
                } else {
                    // For supplementary plane characters, use a UTF-16 surrogate pair.
                    let high = ((unicode_val - 0x10000) >> 10) + 0xD800;
                    let low = ((unicode_val - 0x10000) & 0x3FF) + 0xDC00;
                    let _ = writeln!(cmap, "<{:04X}> <{:04X}{:04X}>", cid, high, low);
                }
            }
            cmap.push_str("endbfchar\n");
        }
    }

    cmap.push_str("endcmap\n");
    cmap.push_str("CMapName currentdict /CMap defineresource pop\n");
    cmap.push_str("end\n");
    cmap.push_str("end\n");

    cmap.into_bytes()
}

/// Build the /W array for CID font widths from (glyph_id, width) pairs.
///
/// Uses the `[cid [w1 w2 ...]]` format for consecutive GIDs and the
/// `[cid1 cid2 w]` format for ranges of identical widths.
fn build_w_array(mut glyph_widths: Vec<(u16, u16)>) -> Vec<PdfObject> {
    if glyph_widths.is_empty() {
        return Vec::new();
    }

    // Sort by glyph ID.
    glyph_widths.sort_by_key(|&(gid, _)| gid);
    glyph_widths.dedup_by_key(|e| e.0);

    let mut w_array: Vec<PdfObject> = Vec::new();

    // Group consecutive GIDs into list entries.
    let mut i = 0;
    while i < glyph_widths.len() {
        let start_gid = glyph_widths[i].0;
        let mut widths_list = vec![PdfObject::Integer(glyph_widths[i].1 as i64)];

        let mut j = i + 1;
        while j < glyph_widths.len() && glyph_widths[j].0 == start_gid + (j - i) as u16 {
            widths_list.push(PdfObject::Integer(glyph_widths[j].1 as i64));
            j += 1;
        }

        // Emit as [start_gid [w1 w2 ...]]
        w_array.push(PdfObject::Integer(start_gid as i64));
        w_array.push(PdfObject::Array(widths_list));

        i = j;
    }

    w_array
}

/// Build a Type0 (composite) font for CJK text and add it to the writer.
///
/// Returns the indirect reference to the Type0 font dictionary, or `None`
/// if `used_chars` is empty.
///
/// # Parameters
///
/// - `writer`: the `PdfWriter` to add objects to
/// - `base_font_name`: the PostScript name of the font (e.g. "NotoSansCJKsc-Regular")
/// - `font_data`: raw TTF/OTF font file bytes
/// - `used_chars`: slice of (unicode_char, glyph_id) pairs for all characters used
///
/// # Created objects
///
/// 1. Font file stream (embedded font data, compressed)
/// 2. Font descriptor dictionary
/// 3. CIDSystemInfo dictionary (inline in CIDFont)
/// 4. CIDFont dictionary (CIDFontType2 for TrueType)
/// 5. ToUnicode CMap stream
/// 6. Type0 font dictionary (the returned reference)
pub fn build_cid_font(
    writer: &mut PdfWriter,
    base_font_name: &str,
    font_data: &[u8],
    used_chars: &[(char, u16)], // (unicode_char, glyph_id)
) -> Option<IndirectRef> {
    if used_chars.is_empty() {
        return None;
    }

    let base_font_bytes = base_font_name.as_bytes().to_vec();

    // 1. Embed the font file as a compressed stream.
    let (mut font_stream_dict, compressed_font_data) = make_stream(font_data, true);
    font_stream_dict.insert(
        b"Length1".to_vec(),
        PdfObject::Integer(font_data.len() as i64),
    );
    let font_file_ref = writer.add_object(PdfObject::Stream {
        dict: font_stream_dict,
        data: compressed_font_data,
    });

    // 2. Create the font descriptor.
    let mut desc_dict = PdfDict::new();
    desc_dict.insert(
        b"Type".to_vec(),
        PdfObject::Name(b"FontDescriptor".to_vec()),
    );
    desc_dict.insert(
        b"FontName".to_vec(),
        PdfObject::Name(base_font_bytes.clone()),
    );
    desc_dict.insert(
        b"Flags".to_vec(),
        PdfObject::Integer(i64::from(super::FontDescriptor::SYMBOLIC)),
    );
    desc_dict.insert(
        b"FontBBox".to_vec(),
        PdfObject::Array(vec![
            PdfObject::Integer(0),
            PdfObject::Integer(-200),
            PdfObject::Integer(1000),
            PdfObject::Integer(800),
        ]),
    );
    desc_dict.insert(b"ItalicAngle".to_vec(), PdfObject::Integer(0));
    desc_dict.insert(b"Ascent".to_vec(), PdfObject::Integer(800));
    desc_dict.insert(b"Descent".to_vec(), PdfObject::Integer(-200));
    desc_dict.insert(b"StemV".to_vec(), PdfObject::Integer(80));
    desc_dict.insert(b"CapHeight".to_vec(), PdfObject::Integer(700));
    desc_dict.insert(
        b"FontFile2".to_vec(),
        PdfObject::Reference(font_file_ref.clone()),
    );

    let desc_ref = writer.add_object(PdfObject::Dict(desc_dict));

    // 3. Build the /W array (glyph widths).
    // For now, use a default width of 1000 for all glyphs.
    // In a full implementation, widths would be read from the font's hmtx table.
    let glyph_widths: Vec<(u16, u16)> = used_chars
        .iter()
        .map(|&(_ch, gid)| (gid, 1000u16))
        .collect();
    let w_array = build_w_array(glyph_widths);

    // 4. Create the CIDFont dictionary.
    let sys_info = CJKOrdering::Identity.system_info();
    let mut cid_font_dict = PdfDict::new();
    cid_font_dict.insert(b"Type".to_vec(), PdfObject::Name(b"Font".to_vec()));
    cid_font_dict.insert(
        b"Subtype".to_vec(),
        PdfObject::Name(b"CIDFontType2".to_vec()),
    );
    cid_font_dict.insert(
        b"BaseFont".to_vec(),
        PdfObject::Name(base_font_bytes.clone()),
    );
    cid_font_dict.insert(
        b"CIDSystemInfo".to_vec(),
        PdfObject::Dict(sys_info.to_pdf_dict()),
    );
    cid_font_dict.insert(
        b"FontDescriptor".to_vec(),
        PdfObject::Reference(desc_ref),
    );
    if !w_array.is_empty() {
        cid_font_dict.insert(b"W".to_vec(), PdfObject::Array(w_array));
    }
    cid_font_dict.insert(b"DW".to_vec(), PdfObject::Integer(1000));
    cid_font_dict.insert(
        b"CIDToGIDMap".to_vec(),
        PdfObject::Name(b"Identity".to_vec()),
    );

    let cid_font_ref = writer.add_object(PdfObject::Dict(cid_font_dict));

    // 5. Generate the ToUnicode CMap.
    let to_unicode_mappings: Vec<(u16, char)> = used_chars
        .iter()
        .map(|&(ch, gid)| (gid, ch))
        .collect();
    let cmap_data = generate_to_unicode_cmap(&to_unicode_mappings);
    let (cmap_dict, cmap_encoded) = make_stream(&cmap_data, true);
    let to_unicode_ref = writer.add_object(PdfObject::Stream {
        dict: cmap_dict,
        data: cmap_encoded,
    });

    // 6. Create the Type0 font dictionary.
    let mut type0_dict = PdfDict::new();
    type0_dict.insert(b"Type".to_vec(), PdfObject::Name(b"Font".to_vec()));
    type0_dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Type0".to_vec()));
    type0_dict.insert(
        b"BaseFont".to_vec(),
        PdfObject::Name(base_font_bytes),
    );
    type0_dict.insert(
        b"Encoding".to_vec(),
        PdfObject::Name(b"Identity-H".to_vec()),
    );
    type0_dict.insert(
        b"DescendantFonts".to_vec(),
        PdfObject::Array(vec![PdfObject::Reference(cid_font_ref)]),
    );
    type0_dict.insert(
        b"ToUnicode".to_vec(),
        PdfObject::Reference(to_unicode_ref),
    );

    Some(writer.add_object(PdfObject::Dict(type0_dict)))
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- CJKOrdering tests ---

    #[test]
    fn test_ordering_system_info_japan1() {
        let info = CJKOrdering::Japan1.system_info();
        assert_eq!(info.registry, "Adobe");
        assert_eq!(info.ordering, "Japan1");
        assert_eq!(info.supplement, 7);
    }

    #[test]
    fn test_ordering_system_info_gb1() {
        let info = CJKOrdering::GB1.system_info();
        assert_eq!(info.registry, "Adobe");
        assert_eq!(info.ordering, "GB1");
        assert_eq!(info.supplement, 5);
    }

    #[test]
    fn test_ordering_system_info_cns1() {
        let info = CJKOrdering::CNS1.system_info();
        assert_eq!(info.registry, "Adobe");
        assert_eq!(info.ordering, "CNS1");
        assert_eq!(info.supplement, 7);
    }

    #[test]
    fn test_ordering_system_info_korea1() {
        let info = CJKOrdering::Korea1.system_info();
        assert_eq!(info.registry, "Adobe");
        assert_eq!(info.ordering, "Korea1");
        assert_eq!(info.supplement, 9);
    }

    #[test]
    fn test_ordering_system_info_identity() {
        let info = CJKOrdering::Identity.system_info();
        assert_eq!(info.registry, "Adobe");
        assert_eq!(info.ordering, "Identity");
        assert_eq!(info.supplement, 0);
    }

    // --- CMap name tests ---

    #[test]
    fn test_cmap_name_japan1() {
        assert_eq!(CJKOrdering::Japan1.cmap_name(), "UniJIS-UTF16-H");
    }

    #[test]
    fn test_cmap_name_gb1() {
        assert_eq!(CJKOrdering::GB1.cmap_name(), "UniGB-UTF16-H");
    }

    #[test]
    fn test_cmap_name_cns1() {
        assert_eq!(CJKOrdering::CNS1.cmap_name(), "UniCNS-UTF16-H");
    }

    #[test]
    fn test_cmap_name_korea1() {
        assert_eq!(CJKOrdering::Korea1.cmap_name(), "UniKS-UTF16-H");
    }

    #[test]
    fn test_cmap_name_identity() {
        assert_eq!(CJKOrdering::Identity.cmap_name(), "Identity-H");
    }

    // --- detect_ordering tests ---

    #[test]
    fn test_detect_japanese_hiragana() {
        // Contains Hiragana characters
        assert_eq!(detect_ordering("\u{3042}\u{3044}\u{3046}"), CJKOrdering::Japan1);
    }

    #[test]
    fn test_detect_japanese_katakana() {
        // Contains Katakana characters
        assert_eq!(detect_ordering("\u{30A2}\u{30A4}\u{30A6}"), CJKOrdering::Japan1);
    }

    #[test]
    fn test_detect_japanese_mixed_with_kanji() {
        // Hiragana + CJK Unified Ideographs: Japanese wins because of Hiragana
        assert_eq!(detect_ordering("\u{3042}\u{4E00}"), CJKOrdering::Japan1);
    }

    #[test]
    fn test_detect_korean_hangul() {
        // Hangul syllables
        assert_eq!(detect_ordering("\u{AC00}\u{D7A3}"), CJKOrdering::Korea1);
    }

    #[test]
    fn test_detect_korean_jamo() {
        // Hangul Jamo
        assert_eq!(detect_ordering("\u{1100}\u{1161}"), CJKOrdering::Korea1);
    }

    #[test]
    fn test_detect_traditional_chinese_bopomofo() {
        // Bopomofo characters
        assert_eq!(detect_ordering("\u{3100}\u{3101}"), CJKOrdering::CNS1);
    }

    #[test]
    fn test_detect_simplified_chinese_cjk_only() {
        // CJK Unified Ideographs without script-specific indicators -> GB1
        assert_eq!(detect_ordering("\u{4E00}\u{4E8C}\u{4E09}"), CJKOrdering::GB1);
    }

    #[test]
    fn test_detect_identity_ascii() {
        assert_eq!(detect_ordering("Hello, World!"), CJKOrdering::Identity);
    }

    #[test]
    fn test_detect_identity_empty() {
        assert_eq!(detect_ordering(""), CJKOrdering::Identity);
    }

    #[test]
    fn test_detect_mixed_japanese_wins() {
        // Hiragana takes priority even if CJK ideographs are present
        assert_eq!(
            detect_ordering("\u{3042}\u{4E00}\u{4E8C}"),
            CJKOrdering::Japan1
        );
    }

    // --- CIDSystemInfo tests ---

    #[test]
    fn test_cid_system_info_to_pdf_dict() {
        let info = CIDSystemInfo {
            registry: "Adobe".into(),
            ordering: "Japan1".into(),
            supplement: 7,
        };
        let dict = info.to_pdf_dict();
        assert_eq!(dict.get_string(b"Registry"), Some(b"Adobe".as_slice()));
        assert_eq!(dict.get_string(b"Ordering"), Some(b"Japan1".as_slice()));
        assert_eq!(dict.get_i64(b"Supplement"), Some(7));
    }

    // --- ToUnicode CMap generation tests ---

    #[test]
    fn test_generate_to_unicode_cmap_basic() {
        let mappings = vec![(1u16, 'A'), (2, 'B'), (3, 'C')];
        let cmap = generate_to_unicode_cmap(&mappings);
        let cmap_str = String::from_utf8(cmap).expect("valid UTF-8");

        assert!(cmap_str.contains("begincmap"));
        assert!(cmap_str.contains("endcmap"));
        assert!(cmap_str.contains("<0000> <FFFF>"));
        assert!(cmap_str.contains("3 beginbfchar"));
        assert!(cmap_str.contains("<0001> <0041>")); // CID 1 -> 'A' (U+0041)
        assert!(cmap_str.contains("<0002> <0042>")); // CID 2 -> 'B'
        assert!(cmap_str.contains("<0003> <0043>")); // CID 3 -> 'C'
        assert!(cmap_str.contains("endbfchar"));
        assert!(cmap_str.contains("/CMapName /Adobe-Identity-UCS def"));
    }

    #[test]
    fn test_generate_to_unicode_cmap_empty() {
        let cmap = generate_to_unicode_cmap(&[]);
        let cmap_str = String::from_utf8(cmap).expect("valid UTF-8");

        assert!(cmap_str.contains("begincmap"));
        assert!(cmap_str.contains("endcmap"));
        // No bfchar section when empty
        assert!(!cmap_str.contains("beginbfchar"));
    }

    #[test]
    fn test_generate_to_unicode_cmap_cjk_chars() {
        // Map some CJK unified ideographs
        let mappings = vec![
            (10u16, '\u{4E00}'), // CJK "one"
            (11, '\u{4E8C}'),    // CJK "two"
        ];
        let cmap = generate_to_unicode_cmap(&mappings);
        let cmap_str = String::from_utf8(cmap).expect("valid UTF-8");

        assert!(cmap_str.contains("<000A> <4E00>"));
        assert!(cmap_str.contains("<000B> <4E8C>"));
    }

    #[test]
    fn test_generate_to_unicode_cmap_supplementary_plane() {
        // U+20000 is in CJK Extension B (supplementary plane)
        let mappings = vec![(100u16, '\u{20000}')];
        let cmap = generate_to_unicode_cmap(&mappings);
        let cmap_str = String::from_utf8(cmap).expect("valid UTF-8");

        // U+20000 -> surrogate pair D840 DC00
        assert!(cmap_str.contains("<0064> <D840DC00>"));
    }

    #[test]
    fn test_generate_to_unicode_cmap_chunking() {
        // Create more than 100 mappings to test chunking
        let mappings: Vec<(u16, char)> = (1..=150)
            .map(|i| (i as u16, char::from_u32(0x4E00 + i as u32).unwrap()))
            .collect();
        let cmap = generate_to_unicode_cmap(&mappings);
        let cmap_str = String::from_utf8(cmap).expect("valid UTF-8");

        // Should have two beginbfchar sections: one with 100 and one with 50
        assert!(cmap_str.contains("100 beginbfchar"));
        assert!(cmap_str.contains("50 beginbfchar"));
    }

    // --- build_w_array tests ---

    #[test]
    fn test_build_w_array_empty() {
        let result = build_w_array(vec![]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_build_w_array_single() {
        let result = build_w_array(vec![(5, 600)]);
        // Should produce: [5 [600]]
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], PdfObject::Integer(5));
        assert_eq!(
            result[1],
            PdfObject::Array(vec![PdfObject::Integer(600)])
        );
    }

    #[test]
    fn test_build_w_array_consecutive() {
        let result = build_w_array(vec![(1, 500), (2, 600), (3, 700)]);
        // Should produce: [1 [500 600 700]]
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], PdfObject::Integer(1));
        assert_eq!(
            result[1],
            PdfObject::Array(vec![
                PdfObject::Integer(500),
                PdfObject::Integer(600),
                PdfObject::Integer(700),
            ])
        );
    }

    #[test]
    fn test_build_w_array_non_consecutive() {
        let result = build_w_array(vec![(1, 500), (5, 600)]);
        // Should produce: [1 [500] 5 [600]]
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], PdfObject::Integer(1));
        assert_eq!(
            result[1],
            PdfObject::Array(vec![PdfObject::Integer(500)])
        );
        assert_eq!(result[2], PdfObject::Integer(5));
        assert_eq!(
            result[3],
            PdfObject::Array(vec![PdfObject::Integer(600)])
        );
    }

    #[test]
    fn test_build_w_array_unsorted_input() {
        // Input is not sorted; function should handle it
        let result = build_w_array(vec![(3, 700), (1, 500), (2, 600)]);
        // After sorting: 1,2,3 are consecutive -> [1 [500 600 700]]
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], PdfObject::Integer(1));
        assert_eq!(
            result[1],
            PdfObject::Array(vec![
                PdfObject::Integer(500),
                PdfObject::Integer(600),
                PdfObject::Integer(700),
            ])
        );
    }

    #[test]
    fn test_build_w_array_duplicates() {
        // Duplicate GIDs should be deduplicated
        let result = build_w_array(vec![(1, 500), (1, 600), (2, 700)]);
        // After dedup: (1, 500), (2, 700) -> [1 [500 700]]
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], PdfObject::Integer(1));
    }

    // --- build_cid_font tests ---

    #[test]
    fn test_build_cid_font_empty_used_chars() {
        let mut writer = PdfWriter::new();
        let result = build_cid_font(&mut writer, "TestFont", b"fake font data", &[]);
        assert!(result.is_none());
        assert!(writer.objects.is_empty());
    }

    #[test]
    fn test_build_cid_font_creates_objects() {
        let mut writer = PdfWriter::new();
        let used_chars = vec![('A', 1u16), ('B', 2), ('\u{4E00}', 100)];
        let result = build_cid_font(&mut writer, "TestCJKFont", b"fake font data", &used_chars);

        assert!(result.is_some());
        let font_ref = result.unwrap();

        // Should have created 5 objects:
        // 1. Font file stream
        // 2. Font descriptor
        // 3. CIDFont dict
        // 4. ToUnicode CMap stream
        // 5. Type0 font dict
        assert_eq!(writer.objects.len(), 5);

        // The returned reference should be the last object (Type0 font)
        assert_eq!(font_ref.obj_num, 5);
        assert_eq!(font_ref.gen_num, 0);

        // Verify the Type0 font dict
        let type0_obj = &writer.objects[4].1;
        if let PdfObject::Dict(dict) = type0_obj {
            assert_eq!(dict.get_name(b"Type"), Some(b"Font".as_slice()));
            assert_eq!(dict.get_name(b"Subtype"), Some(b"Type0".as_slice()));
            assert_eq!(dict.get_name(b"BaseFont"), Some(b"TestCJKFont".as_slice()));
            assert_eq!(dict.get_name(b"Encoding"), Some(b"Identity-H".as_slice()));
            assert!(dict.get(b"DescendantFonts").is_some());
            assert!(dict.get(b"ToUnicode").is_some());
        } else {
            panic!("Expected Type0 font to be a Dict");
        }

        // Verify the CIDFont dict
        let cid_font_obj = &writer.objects[2].1;
        if let PdfObject::Dict(dict) = cid_font_obj {
            assert_eq!(dict.get_name(b"Subtype"), Some(b"CIDFontType2".as_slice()));
            assert_eq!(
                dict.get_name(b"CIDToGIDMap"),
                Some(b"Identity".as_slice())
            );
            assert!(dict.get(b"W").is_some());
            assert_eq!(dict.get_i64(b"DW"), Some(1000));
        } else {
            panic!("Expected CIDFont to be a Dict");
        }

        // Verify the font descriptor
        let desc_obj = &writer.objects[1].1;
        if let PdfObject::Dict(dict) = desc_obj {
            assert_eq!(
                dict.get_name(b"Type"),
                Some(b"FontDescriptor".as_slice())
            );
            assert_eq!(
                dict.get_name(b"FontName"),
                Some(b"TestCJKFont".as_slice())
            );
            assert!(dict.get(b"FontFile2").is_some());
        } else {
            panic!("Expected FontDescriptor to be a Dict");
        }
    }
}
