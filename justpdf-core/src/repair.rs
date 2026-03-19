//! PDF repair module — rebuilds the cross-reference table by scanning
//! for object definitions when the normal xref/trailer structure is
//! damaged or missing.

use std::collections::HashMap;

use crate::error::{JustPdfError, Result};
use crate::object::{self, IndirectRef, PdfDict, PdfObject};
use crate::parser::PdfDocument;
use crate::tokenizer::Tokenizer;
use crate::xref::{Xref, XrefEntry};

/// Rebuild the cross-reference table by scanning the raw bytes for
/// `N M obj` patterns and locating the trailer dictionary.
///
/// This is the core recovery routine: it walks every byte looking for
/// object headers, records each object's offset, then tries to find
/// (or synthesise) a trailer dictionary so that a usable [`Xref`] can
/// be returned.
pub fn rebuild_xref(data: &[u8]) -> Result<Xref> {
    let entries = scan_object_headers(data);

    if entries.is_empty() {
        return Err(JustPdfError::InvalidXref {
            offset: 0,
            detail: "no objects found during repair scan".into(),
        });
    }

    // Try to find a trailer dictionary the traditional way.
    let trailer = find_trailer_dict(data).or_else(|_| synthesise_trailer(data, &entries))?;

    let max_obj = entries.keys().copied().max().unwrap_or(0);

    let mut xref = Xref::new();
    for (&obj_num, &(offset, gen_num)) in &entries {
        xref.entries.insert(
            obj_num,
            XrefEntry::InUse {
                offset: offset as u64,
                gen_num,
            },
        );
    }

    // Ensure /Size is present in the trailer.
    let mut trailer = trailer;
    if trailer.get_i64(b"Size").is_none() {
        trailer.insert(
            b"Size".to_vec(),
            PdfObject::Integer((max_obj + 1) as i64),
        );
    }

    xref.trailer = trailer;
    Ok(xref)
}

/// Try normal parsing first; if it fails, fall back to [`rebuild_xref`].
pub fn repair_document(data: Vec<u8>) -> Result<PdfDocument> {
    // Happy path — normal parsing.
    match PdfDocument::from_bytes(data.clone()) {
        Ok(doc) => return Ok(doc),
        Err(_) => {}
    }

    // Fallback — repair.
    PdfDocument::from_bytes_with_repair(data)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Scan the entire file for lines matching `\d+ \d+ obj`.
///
/// Returns a map from object number to `(byte_offset, generation)`.
/// If the same object number appears more than once, the *last*
/// occurrence wins (mirrors incremental-update semantics).
fn scan_object_headers(data: &[u8]) -> HashMap<u32, (usize, u16)> {
    let mut map: HashMap<u32, (usize, u16)> = HashMap::new();
    let len = data.len();
    let mut i = 0;

    while i < len {
        // Fast-skip: we only care about positions that are at the start of
        // a line (pos 0 or preceded by \n or \r).
        if i != 0 && data[i - 1] != b'\n' && data[i - 1] != b'\r' {
            // Advance to next line boundary.
            while i < len && data[i] != b'\n' && data[i] != b'\r' {
                i += 1;
            }
            // Skip the newline character(s).
            while i < len && (data[i] == b'\n' || data[i] == b'\r') {
                i += 1;
            }
            continue;
        }

        // Try to match `<digits> <digits> obj` at position i.
        if let Some((obj_num, gen_num, after)) = match_obj_header(data, i) {
            map.insert(obj_num, (i, gen_num));
            i = after;
        } else {
            // Advance to next line.
            while i < len && data[i] != b'\n' && data[i] != b'\r' {
                i += 1;
            }
            while i < len && (data[i] == b'\n' || data[i] == b'\r') {
                i += 1;
            }
        }
    }

    map
}

/// Try to match `<obj_num> <gen_num> obj` starting at `pos`.
/// Returns `(obj_num, gen_num, byte_after_keyword)` on success.
fn match_obj_header(data: &[u8], pos: usize) -> Option<(u32, u16, usize)> {
    let len = data.len();
    let mut i = pos;

    // Skip optional leading whitespace (spaces/tabs, NOT newlines — we
    // already ensured we are at a line boundary).
    while i < len && (data[i] == b' ' || data[i] == b'\t') {
        i += 1;
    }

    // First number: object number.
    let num_start = i;
    while i < len && data[i].is_ascii_digit() {
        i += 1;
    }
    if i == num_start || i >= len {
        return None;
    }
    let obj_num: u32 = std::str::from_utf8(&data[num_start..i]).ok()?.parse().ok()?;

    // Whitespace between numbers.
    if i >= len || data[i] != b' ' {
        return None;
    }
    while i < len && data[i] == b' ' {
        i += 1;
    }

    // Second number: generation number.
    let gen_start = i;
    while i < len && data[i].is_ascii_digit() {
        i += 1;
    }
    if i == gen_start || i >= len {
        return None;
    }
    let gen_num: u16 = std::str::from_utf8(&data[gen_start..i]).ok()?.parse().ok()?;

    // Whitespace before `obj`.
    if i >= len || data[i] != b' ' {
        return None;
    }
    while i < len && data[i] == b' ' {
        i += 1;
    }

    // Keyword `obj` followed by whitespace / EOF.
    if i + 3 > len {
        return None;
    }
    if &data[i..i + 3] != b"obj" {
        return None;
    }
    let after = i + 3;
    // `obj` must be followed by a delimiter (whitespace, <, [, /) or EOF.
    if after < len {
        let ch = data[after];
        if !(ch == b' '
            || ch == b'\t'
            || ch == b'\n'
            || ch == b'\r'
            || ch == b'<'
            || ch == b'['
            || ch == b'/')
        {
            return None; // e.g. "object" — not the keyword we want.
        }
    }

    Some((obj_num, gen_num, after))
}

/// Scan backward for the `trailer` keyword and parse the dictionary
/// that follows it.
fn find_trailer_dict(data: &[u8]) -> Result<PdfDict> {
    let needle = b"trailer";
    // Search the last 4 KiB (covers most files, even with multiple
    // incremental updates we only need the *last* trailer).
    let search_len = data.len().min(4096);
    let search_start = data.len() - search_len;

    for i in (search_start..data.len().saturating_sub(needle.len())).rev() {
        if &data[i..i + needle.len()] == needle {
            // Skip "trailer" + whitespace, then parse the dict.
            let after = i + needle.len();
            let mut tokenizer = Tokenizer::new_at(data, after);
            if let Ok(obj) = object::parse_object(&mut tokenizer) {
                if let PdfObject::Dict(d) = obj {
                    return Ok(d);
                }
            }
        }
    }

    Err(JustPdfError::TrailerNotFound)
}

/// When no explicit trailer can be found, build a minimal one by
/// locating the catalog object (an object whose dictionary contains
/// `/Type /Catalog`).
fn synthesise_trailer(
    data: &[u8],
    entries: &HashMap<u32, (usize, u16)>,
) -> Result<PdfDict> {
    let mut root_ref: Option<IndirectRef> = None;

    for (&obj_num, &(offset, gen_num)) in entries {
        if let Some(dict) = try_parse_dict_at(data, offset) {
            if dict.get_name(b"Type") == Some(b"Catalog") {
                root_ref = Some(IndirectRef { obj_num, gen_num });
                break;
            }
        }
    }

    let root = root_ref.ok_or(JustPdfError::TrailerNotFound)?;

    let max_obj = entries.keys().copied().max().unwrap_or(0);

    let mut trailer = PdfDict::new();
    trailer.insert(
        b"Root".to_vec(),
        PdfObject::Reference(root),
    );
    trailer.insert(
        b"Size".to_vec(),
        PdfObject::Integer((max_obj + 1) as i64),
    );

    Ok(trailer)
}

/// Attempt to parse the indirect object at `offset` and return its
/// dictionary if the top-level value is a `Dict` (ignoring streams
/// for simplicity).
fn try_parse_dict_at(data: &[u8], offset: usize) -> Option<PdfDict> {
    let mut tokenizer = Tokenizer::new_at(data, offset);
    let (_iref, obj) = object::parse_indirect_object(&mut tokenizer).ok()?;
    match obj {
        PdfObject::Dict(d) => Some(d),
        PdfObject::Stream { dict, .. } => Some(dict),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Integration: PdfDocument::from_bytes_with_repair
// ---------------------------------------------------------------------------

impl PdfDocument {
    /// Parse a PDF from bytes, falling back to xref repair if the
    /// normal cross-reference table or trailer is damaged.
    ///
    /// This tries [`PdfDocument::from_bytes`] first.  If that fails
    /// the file is scanned for object definitions and a synthetic xref
    /// is built via [`rebuild_xref`].
    pub fn from_bytes_with_repair(data: Vec<u8>) -> Result<Self> {
        // Try the normal path first.
        match Self::from_bytes(data.clone()) {
            Ok(doc) => return Ok(doc),
            Err(_normal_err) => {}
        }

        // Repair path: rebuild xref by scanning.
        let xref = rebuild_xref(&data)?;
        let version = parse_version_tolerant(&data);

        Ok(Self::from_raw_parts(data, xref, version))
    }
}

/// Parse PDF version, returning (1, 4) as a safe default when the
/// header is missing or corrupt.
fn parse_version_tolerant(data: &[u8]) -> (u8, u8) {
    let needle = b"%PDF-";
    let search_len = data.len().min(1024);
    for i in 0..search_len.saturating_sub(needle.len() + 3) {
        if &data[i..i + needle.len()] == needle {
            let major = data.get(i + 5).copied().unwrap_or(0);
            let dot = data.get(i + 6).copied().unwrap_or(0);
            let minor = data.get(i + 7).copied().unwrap_or(0);
            if major.is_ascii_digit() && dot == b'.' && minor.is_ascii_digit() {
                return (major - b'0', minor - b'0');
            }
        }
    }
    (1, 4) // safe default
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid PDF for testing.
    fn build_minimal_pdf() -> Vec<u8> {
        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.4\n");

        let obj1_offset = pdf.len();
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

        let obj2_offset = pdf.len();
        pdf.extend_from_slice(
            b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n",
        );

        let obj3_offset = pdf.len();
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n",
        );

        let xref_offset = pdf.len();
        pdf.extend_from_slice(b"xref\n");
        pdf.extend_from_slice(b"0 4\n");
        pdf.extend_from_slice(b"0000000000 65535 f \r\n");
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj1_offset).as_bytes());
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj2_offset).as_bytes());
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj3_offset).as_bytes());

        pdf.extend_from_slice(b"trailer\n<< /Size 4 /Root 1 0 R >>\n");
        pdf.extend_from_slice(format!("startxref\n{xref_offset}\n%%EOF\n").as_bytes());

        pdf
    }

    // ------------------------------------------------------------------
    // rebuild_xref on a valid PDF — compare with normal parsing
    // ------------------------------------------------------------------

    #[test]
    fn test_rebuild_xref_matches_normal() {
        let data = build_minimal_pdf();

        // Normal load
        let normal_xref = crate::xref::load_xref(&data).unwrap();

        // Repair load
        let repaired_xref = rebuild_xref(&data).unwrap();

        // Both must contain objects 1, 2, 3 as InUse with the same offsets.
        for obj_num in 1u32..=3 {
            let normal_entry = normal_xref.get(obj_num).unwrap();
            let repair_entry = repaired_xref.get(obj_num).unwrap();
            match (normal_entry, repair_entry) {
                (
                    XrefEntry::InUse {
                        offset: o1,
                        gen_num: g1,
                    },
                    XrefEntry::InUse {
                        offset: o2,
                        gen_num: g2,
                    },
                ) => {
                    assert_eq!(o1, o2, "offset mismatch for obj {obj_num}");
                    assert_eq!(g1, g2, "gen mismatch for obj {obj_num}");
                }
                _ => panic!("unexpected entry type for obj {obj_num}"),
            }
        }

        // Trailer must have /Root
        assert!(repaired_xref.trailer.get_ref(b"Root").is_some());
    }

    // ------------------------------------------------------------------
    // Truncated trailer — trailer keyword removed, rebuild from catalog
    // ------------------------------------------------------------------

    #[test]
    fn test_rebuild_xref_truncated_trailer() {
        let mut data = build_minimal_pdf();

        // Remove everything from `xref` onward so there is no trailer.
        if let Some(pos) = data
            .windows(4)
            .position(|w| w == b"xref")
        {
            data.truncate(pos);
        }

        // Normal parsing must fail.
        assert!(PdfDocument::from_bytes(data.clone()).is_err());

        // Repair must succeed by synthesising the trailer from the
        // catalog object.
        let repaired = rebuild_xref(&data).unwrap();
        assert!(repaired.get(1).is_some());
        assert!(repaired.get(2).is_some());
        assert!(repaired.get(3).is_some());

        // The synthetic trailer must reference the catalog.
        let root = repaired.trailer.get_ref(b"Root").expect("/Root missing");
        assert_eq!(root.obj_num, 1);
    }

    // ------------------------------------------------------------------
    // Detecting catalog from objects
    // ------------------------------------------------------------------

    #[test]
    fn test_detect_catalog_object() {
        // Build a PDF body with no trailer at all.
        let mut data = Vec::new();
        data.extend_from_slice(b"%PDF-1.7\n");
        data.extend_from_slice(
            b"5 0 obj\n<< /Type /Pages /Kids [] /Count 0 >>\nendobj\n",
        );
        data.extend_from_slice(
            b"10 0 obj\n<< /Type /Catalog /Pages 5 0 R >>\nendobj\n",
        );

        let repaired = rebuild_xref(&data).unwrap();

        // The repaired xref should find both objects.
        assert!(repaired.get(5).is_some());
        assert!(repaired.get(10).is_some());

        // /Root should point at object 10 (the Catalog).
        let root = repaired.trailer.get_ref(b"Root").unwrap();
        assert_eq!(root.obj_num, 10);
    }

    // ------------------------------------------------------------------
    // scan_object_headers edge cases
    // ------------------------------------------------------------------

    #[test]
    fn test_scan_ignores_non_obj_keywords() {
        // "object" should not match — only "obj" followed by a delimiter.
        let data = b"%PDF-1.4\n1 0 object\n2 0 obj\n<< >>\nendobj\n";
        let entries = scan_object_headers(data);
        assert!(!entries.contains_key(&1));
        assert!(entries.contains_key(&2));
    }

    #[test]
    fn test_scan_generation_number() {
        let data = b"%PDF-1.4\n7 3 obj\n<< /Foo /Bar >>\nendobj\n";
        let entries = scan_object_headers(data);
        let (_, gen_val) = entries.get(&7).expect("object 7 not found");
        assert_eq!(*gen_val, 3);
    }

    // ------------------------------------------------------------------
    // repair_document (convenience wrapper)
    // ------------------------------------------------------------------

    #[test]
    fn test_repair_document_valid_pdf() {
        let data = build_minimal_pdf();
        let doc = repair_document(data).unwrap();
        assert_eq!(doc.version, (1, 4));
        assert!(doc.object_count() > 0);
    }

    #[test]
    fn test_repair_document_damaged_pdf() {
        let mut data = build_minimal_pdf();

        // Corrupt the xref region.
        if let Some(pos) = data.windows(4).position(|w| w == b"xref") {
            data.truncate(pos);
        }

        let doc = repair_document(data).unwrap();
        assert!(doc.object_count() > 0);
    }

    // ------------------------------------------------------------------
    // from_bytes_with_repair
    // ------------------------------------------------------------------

    #[test]
    fn test_from_bytes_with_repair_valid() {
        let data = build_minimal_pdf();
        let doc = PdfDocument::from_bytes_with_repair(data).unwrap();
        assert_eq!(doc.version, (1, 4));
    }

    #[test]
    fn test_from_bytes_with_repair_damaged() {
        let mut data = build_minimal_pdf();
        if let Some(pos) = data.windows(4).position(|w| w == b"xref") {
            data.truncate(pos);
        }
        let doc = PdfDocument::from_bytes_with_repair(data).unwrap();
        assert!(doc.object_count() >= 3);
    }
}
