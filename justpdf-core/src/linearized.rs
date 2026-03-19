//! Linearized PDF detection and parameter parsing (PDF spec section 7.4, detection only).
//!
//! This module detects whether a PDF is linearized and extracts the linearization
//! parameters from the linearization dictionary. It does NOT generate linearized PDFs
//! (deferred to Phase 8).

use crate::object::{parse_indirect_object, PdfObject};
use crate::parser::PdfDocument;
use crate::tokenizer::Tokenizer;

/// Linearization parameters from the linearization dictionary.
#[derive(Debug, Clone)]
pub struct LinearizationParams {
    /// File length (/L).
    pub file_length: i64,
    /// Hint stream offset (/H first element).
    pub hint_offset: i64,
    /// Hint stream length (/H second element).
    pub hint_length: i64,
    /// First page object number (/O).
    pub first_page_obj_num: u32,
    /// Offset of end of first page (/E).
    pub end_of_first_page: i64,
    /// Number of pages (/N).
    pub page_count: i64,
    /// Offset of main xref table (/T).
    pub main_xref_offset: i64,
    /// Linearization version, typically 1.0.
    pub version: f64,
}

/// Find the byte offset just past the PDF header line (`%PDF-x.y` plus its line ending).
///
/// Returns `None` if no valid header is found in the first 1024 bytes.
fn skip_header(data: &[u8]) -> Option<usize> {
    let search_len = data.len().min(1024);
    let needle = b"%PDF-";

    for i in 0..search_len.saturating_sub(needle.len()) {
        if data[i..].starts_with(needle) {
            // Skip past the header line (find the next line break).
            let mut pos = i + needle.len();
            // Skip the rest of the header line (version digits etc.)
            while pos < data.len() && data[pos] != b'\n' && data[pos] != b'\r' {
                pos += 1;
            }
            // Skip the line ending itself.
            if pos < data.len() && data[pos] == b'\r' {
                pos += 1;
            }
            if pos < data.len() && data[pos] == b'\n' {
                pos += 1;
            }
            return Some(pos);
        }
    }
    None
}

/// Skip any comment lines (lines starting with `%`) after the header.
fn skip_comments(data: &[u8], mut pos: usize) -> usize {
    loop {
        // Skip whitespace between lines.
        while pos < data.len()
            && (data[pos] == b' ' || data[pos] == b'\t' || data[pos] == b'\r' || data[pos] == b'\n')
        {
            pos += 1;
        }
        if pos < data.len() && data[pos] == b'%' {
            // Skip comment line.
            while pos < data.len() && data[pos] != b'\n' && data[pos] != b'\r' {
                pos += 1;
            }
        } else {
            break;
        }
    }
    pos
}

/// Detect whether the given PDF data represents a linearized PDF, and if so,
/// parse and return the linearization parameters.
///
/// This checks if the first indirect object in the file contains a `/Linearized` key.
pub fn detect_linearization(data: &[u8]) -> Option<LinearizationParams> {
    let header_end = skip_header(data)?;
    let obj_start = skip_comments(data, header_end);

    if obj_start >= data.len() {
        return None;
    }

    let mut tokenizer = Tokenizer::new_at(data, obj_start);
    let (_iref, obj) = parse_indirect_object(&mut tokenizer).ok()?;

    let dict = match &obj {
        PdfObject::Dict(d) => d,
        _ => return None,
    };

    // Check for the /Linearized key.
    if !dict.contains_key(b"Linearized") {
        return None;
    }

    let version = dict.get_f64(b"Linearized")?;
    let file_length = dict.get_i64(b"L")?;
    let first_page_obj_num = dict.get_i64(b"O")? as u32;
    let end_of_first_page = dict.get_i64(b"E")?;
    let page_count = dict.get_i64(b"N")?;
    let main_xref_offset = dict.get_i64(b"T")?;

    // /H is an array of 2 integers: [offset length].
    let h_array = dict.get_array(b"H")?;
    if h_array.len() < 2 {
        return None;
    }
    let hint_offset = h_array[0].as_i64()?;
    let hint_length = h_array[1].as_i64()?;

    Some(LinearizationParams {
        file_length,
        hint_offset,
        hint_length,
        first_page_obj_num,
        end_of_first_page,
        page_count,
        main_xref_offset,
        version,
    })
}

/// Simple check: returns `true` if the PDF data appears to be linearized.
pub fn is_linearized(data: &[u8]) -> bool {
    detect_linearization(data).is_some()
}

/// Alternative API that works from a parsed `PdfDocument`.
///
/// Finds the first object (usually obj 1) and checks for `/Linearized`.
/// This re-parses from the raw data since the linearization dict is always the
/// first indirect object in the file.
pub fn read_linearization(doc: &mut PdfDocument) -> Option<LinearizationParams> {
    detect_linearization(doc.raw_data())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a minimal linearized PDF byte stream.
    fn make_linearized_pdf(
        file_length: i64,
        hint_offset: i64,
        hint_length: i64,
        first_page_obj: u32,
        end_first_page: i64,
        page_count: i64,
        main_xref: i64,
    ) -> Vec<u8> {
        format!(
            "%PDF-1.7\n\
             1 0 obj\n\
             << /Linearized 1.0 /L {file_length} /H [{hint_offset} {hint_length}] \
             /O {first_page_obj} /E {end_first_page} /N {page_count} /T {main_xref} >>\n\
             endobj\n"
        )
        .into_bytes()
    }

    #[test]
    fn detect_linearized_pdf() {
        let data = make_linearized_pdf(12345, 200, 50, 5, 1000, 10, 9000);
        let params = detect_linearization(&data).expect("should detect linearization");
        assert_eq!(params.file_length, 12345);
        assert_eq!(params.hint_offset, 200);
        assert_eq!(params.hint_length, 50);
        assert_eq!(params.first_page_obj_num, 5);
        assert_eq!(params.end_of_first_page, 1000);
        assert_eq!(params.page_count, 10);
        assert_eq!(params.main_xref_offset, 9000);
        assert!((params.version - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn is_linearized_true() {
        let data = make_linearized_pdf(500, 100, 30, 2, 400, 3, 450);
        assert!(is_linearized(&data));
    }

    #[test]
    fn non_linearized_pdf_returns_none() {
        let data = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n";
        assert!(detect_linearization(data).is_none());
        assert!(!is_linearized(data));
    }

    #[test]
    fn non_dict_first_object_returns_none() {
        let data = b"%PDF-1.4\n1 0 obj\n42\nendobj\n";
        assert!(detect_linearization(data).is_none());
    }

    #[test]
    fn header_with_comment_line() {
        // Some PDFs have a binary comment after the header.
        let data = b"%PDF-1.5\n%\xE2\xE3\xCF\xD3\n\
            1 0 obj\n\
            << /Linearized 1.0 /L 5000 /H [100 20] /O 3 /E 800 /N 5 /T 4500 >>\n\
            endobj\n";
        let params = detect_linearization(data).expect("should detect through comment");
        assert_eq!(params.file_length, 5000);
        assert_eq!(params.page_count, 5);
    }

    #[test]
    fn short_input_does_not_panic() {
        assert!(detect_linearization(b"").is_none());
        assert!(detect_linearization(b"%PDF").is_none());
        assert!(detect_linearization(b"%PDF-1.4\n").is_none());
        assert!(detect_linearization(b"%PDF-1.4\n1").is_none());
    }

    #[test]
    fn truncated_dict_does_not_panic() {
        let data = b"%PDF-1.4\n1 0 obj\n<< /Linearized 1.0 /L";
        assert!(detect_linearization(data).is_none());
    }

    #[test]
    fn missing_required_key_returns_none() {
        // Has /Linearized but missing /L.
        let data = b"%PDF-1.4\n1 0 obj\n\
            << /Linearized 1.0 /H [100 20] /O 3 /E 800 /N 5 /T 4500 >>\n\
            endobj\n";
        assert!(detect_linearization(data).is_none());
    }

    #[test]
    fn h_array_too_short_returns_none() {
        let data = b"%PDF-1.4\n1 0 obj\n\
            << /Linearized 1.0 /L 5000 /H [100] /O 3 /E 800 /N 5 /T 4500 >>\n\
            endobj\n";
        assert!(detect_linearization(data).is_none());
    }

    #[test]
    fn parse_all_params_correctly() {
        let data = make_linearized_pdf(999999, 512, 128, 7, 2048, 42, 88888);
        let params = detect_linearization(&data).unwrap();
        assert_eq!(params.file_length, 999999);
        assert_eq!(params.hint_offset, 512);
        assert_eq!(params.hint_length, 128);
        assert_eq!(params.first_page_obj_num, 7);
        assert_eq!(params.end_of_first_page, 2048);
        assert_eq!(params.page_count, 42);
        assert_eq!(params.main_xref_offset, 88888);
    }

    #[test]
    fn version_as_real() {
        let data = b"%PDF-1.7\n1 0 obj\n\
            << /Linearized 1.0 /L 100 /H [10 5] /O 1 /E 50 /N 1 /T 80 >>\n\
            endobj\n";
        let params = detect_linearization(data).unwrap();
        assert!((params.version - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn version_as_integer() {
        // Some generators write /Linearized 1 (integer) instead of 1.0.
        let data = b"%PDF-1.7\n1 0 obj\n\
            << /Linearized 1 /L 100 /H [10 5] /O 1 /E 50 /N 1 /T 80 >>\n\
            endobj\n";
        let params = detect_linearization(data).unwrap();
        assert!((params.version - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn read_linearization_from_document() {
        let data = make_linearized_pdf(5000, 100, 20, 3, 800, 5, 4500);
        // PdfDocument::from_bytes requires valid xref/trailer, so we test
        // detect_linearization directly for the raw-data path.
        let params = detect_linearization(&data).unwrap();
        assert_eq!(params.page_count, 5);
    }
}
