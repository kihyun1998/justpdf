use std::path::Path;

use justpdf_core::{IndirectRef, JustPdfError, PdfDocument, PdfObject};
use justpdf_core::page::collect_pages;
use justpdf_core::text;
use justpdf_core::text::search;
use justpdf_core::text::format::{self, OutputFormat};

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

// ============================================================
// Positive tests
// ============================================================

#[test]
fn test_open_minimal_pdf() {
    let mut doc = PdfDocument::open(&fixture("minimal.pdf")).unwrap();
    assert_eq!(doc.version, (1, 4));
    assert!(doc.object_count() > 0);

    let cat_ref = doc.catalog_ref().unwrap().clone();
    let catalog = doc.resolve(&cat_ref).unwrap();
    assert_eq!(
        catalog.as_dict().unwrap().get_name(b"Type"),
        Some(b"Catalog".as_slice())
    );
}

#[test]
fn test_two_pages() {
    let mut doc = PdfDocument::open(&fixture("two_pages.pdf")).unwrap();

    let cat_ref = doc.catalog_ref().unwrap().clone();
    let catalog = doc.resolve(&cat_ref).unwrap().clone();
    let pages_ref = catalog
        .as_dict()
        .unwrap()
        .get_ref(b"Pages")
        .unwrap()
        .clone();
    let pages = doc.resolve(&pages_ref).unwrap();
    assert_eq!(pages.as_dict().unwrap().get_i64(b"Count"), Some(2));
}

#[test]
fn test_with_text_stream() {
    let mut doc = PdfDocument::open(&fixture("with_text.pdf")).unwrap();

    // Resolve the content stream (obj 4)
    let iref = IndirectRef {
        obj_num: 4,
        gen_num: 0,
    };
    let obj = doc.resolve(&iref).unwrap().clone();
    let (dict, raw_data) = obj.as_stream().unwrap();
    let decoded = doc.decode_stream(dict, raw_data).unwrap();
    let text = std::str::from_utf8(&decoded).unwrap();
    assert!(text.contains("Hello World"));
}

#[test]
fn test_compressed_stream() {
    let mut doc = PdfDocument::open(&fixture("compressed_stream.pdf")).unwrap();

    let iref = IndirectRef {
        obj_num: 4,
        gen_num: 0,
    };
    let obj = doc.resolve(&iref).unwrap().clone();
    let (dict, raw_data) = obj.as_stream().unwrap();

    // Verify it's compressed
    assert_eq!(dict.get_name(b"Filter"), Some(b"FlateDecode".as_slice()));

    let decoded = doc.decode_stream(dict, raw_data).unwrap();
    let text = std::str::from_utf8(&decoded).unwrap();
    assert!(text.contains("Compressed content stream"));
}

#[test]
fn test_ascii_hex_stream() {
    let mut doc = PdfDocument::open(&fixture("ascii_hex_stream.pdf")).unwrap();

    let iref = IndirectRef {
        obj_num: 4,
        gen_num: 0,
    };
    let obj = doc.resolve(&iref).unwrap().clone();
    let (dict, raw_data) = obj.as_stream().unwrap();

    assert_eq!(dict.get_name(b"Filter"), Some(b"ASCIIHexDecode".as_slice()));

    let decoded = doc.decode_stream(dict, raw_data).unwrap();
    let text = std::str::from_utf8(&decoded).unwrap();
    assert!(text.contains("ASCIIHex encoded"));
}

#[test]
fn test_incremental_update() {
    let mut doc = PdfDocument::open(&fixture("incremental.pdf")).unwrap();

    // Should have Info dict from incremental update
    let trailer = doc.trailer();
    assert!(trailer.get_ref(b"Info").is_some());

    // Resolve Info dict (obj 4)
    let info_ref = trailer.get_ref(b"Info").unwrap().clone();
    let info = doc.resolve(&info_ref).unwrap();
    let info_dict = info.as_dict().unwrap();

    // Check Title from incremental update
    assert_eq!(
        info_dict.get(b"Title"),
        Some(&PdfObject::String(b"Test Document".to_vec()))
    );
    assert_eq!(
        info_dict.get(b"Author"),
        Some(&PdfObject::String(b"justpdf".to_vec()))
    );

    // Original objects should still be accessible
    let cat_ref = doc.catalog_ref().unwrap().clone();
    let catalog = doc.resolve(&cat_ref).unwrap();
    assert_eq!(
        catalog.as_dict().unwrap().get_name(b"Type"),
        Some(b"Catalog".as_slice())
    );
}

// ============================================================
// Text extraction tests
// ============================================================

#[test]
fn test_extract_text_hello_world() {
    let mut doc = PdfDocument::open(&fixture("with_text.pdf")).unwrap();
    let pages = collect_pages(&mut doc).unwrap();
    assert_eq!(pages.len(), 1);

    let page_text = text::extract_page_text(&mut doc, &pages[0]).unwrap();

    // Should have extracted characters
    assert!(!page_text.chars.is_empty());

    // Check plain text contains "Hello World"
    let plain = page_text.plain_text();
    assert!(
        plain.contains("Hello World"),
        "Expected 'Hello World' in: {plain:?}"
    );

    // Check character positions: first char at (72, 720), font 24pt
    let first_char = &page_text.chars[0];
    assert_eq!(first_char.unicode, "H");
    assert!((first_char.x - 72.0).abs() < 0.1);
    assert!((first_char.y - 720.0).abs() < 0.1);
    assert!((first_char.font_size - 24.0).abs() < 0.1);
}

#[test]
fn test_extract_text_compressed() {
    let mut doc = PdfDocument::open(&fixture("compressed_stream.pdf")).unwrap();
    let pages = collect_pages(&mut doc).unwrap();
    assert_eq!(pages.len(), 1);

    let page_text = text::extract_page_text(&mut doc, &pages[0]).unwrap();
    let plain = page_text.plain_text();
    assert!(
        plain.contains("Compressed content stream"),
        "Expected 'Compressed content stream' in: {plain:?}"
    );
}

#[test]
fn test_extract_text_all_pages() {
    let mut doc = PdfDocument::open(&fixture("with_text.pdf")).unwrap();
    let result = text::extract_all_text_string(&mut doc).unwrap();
    assert!(result.contains("Hello World"));
}

#[test]
fn test_extract_text_empty_page() {
    let mut doc = PdfDocument::open(&fixture("minimal.pdf")).unwrap();
    let pages = collect_pages(&mut doc).unwrap();
    assert_eq!(pages.len(), 1);

    let page_text = text::extract_page_text(&mut doc, &pages[0]).unwrap();
    assert!(page_text.chars.is_empty());
    assert_eq!(page_text.plain_text(), "");
}

#[test]
fn test_extract_text_word_grouping() {
    let mut doc = PdfDocument::open(&fixture("with_text.pdf")).unwrap();
    let pages = collect_pages(&mut doc).unwrap();
    let page_text = text::extract_page_text(&mut doc, &pages[0]).unwrap();

    // "Hello World" → should have 2 words
    assert!(!page_text.lines.is_empty());
    let first_line = &page_text.lines[0];
    assert_eq!(first_line.words.len(), 2);
    assert_eq!(first_line.words[0].text, "Hello");
    assert_eq!(first_line.words[1].text, "World");
}

// ============================================================
// Text search tests
// ============================================================

#[test]
fn test_search_exact_in_pdf() {
    let mut doc = PdfDocument::open(&fixture("with_text.pdf")).unwrap();
    let pages = text::extract_all_text(&mut doc).unwrap();
    let results = search::search_exact(&pages, "Hello");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].page_index, 0);
    assert_eq!(results[0].matched_text, "Hello");
    assert!(results[0].quad.x0 >= 72.0);
}

#[test]
fn test_search_case_insensitive_in_pdf() {
    let mut doc = PdfDocument::open(&fixture("with_text.pdf")).unwrap();
    let pages = text::extract_all_text(&mut doc).unwrap();
    let results = search::search_case_insensitive(&pages, "hello world");
    assert_eq!(results.len(), 1);
}

#[test]
fn test_search_no_match_in_pdf() {
    let mut doc = PdfDocument::open(&fixture("with_text.pdf")).unwrap();
    let pages = text::extract_all_text(&mut doc).unwrap();
    let results = search::search_exact(&pages, "nonexistent");
    assert!(results.is_empty());
}

#[test]
fn test_search_regex_in_pdf() {
    let mut doc = PdfDocument::open(&fixture("with_text.pdf")).unwrap();
    let pages = text::extract_all_text(&mut doc).unwrap();
    let results = search::search_regex(&pages, "H\\w+").unwrap();
    assert!(!results.is_empty());
    assert!(results[0].matched_text.starts_with('H'));
}

#[test]
fn test_search_empty_page() {
    let mut doc = PdfDocument::open(&fixture("minimal.pdf")).unwrap();
    let pages = text::extract_all_text(&mut doc).unwrap();
    let results = search::search_exact(&pages, "anything");
    assert!(results.is_empty());
}

// ============================================================
// Text format tests
// ============================================================

#[test]
fn test_format_plain_text() {
    let mut doc = PdfDocument::open(&fixture("with_text.pdf")).unwrap();
    let pages = text::extract_all_text(&mut doc).unwrap();
    let plain = format::format_page(&pages[0], OutputFormat::PlainText);
    assert!(plain.contains("Hello World"));
}

#[test]
fn test_format_html() {
    let mut doc = PdfDocument::open(&fixture("with_text.pdf")).unwrap();
    let pages = text::extract_all_text(&mut doc).unwrap();
    let html = format::format_page(&pages[0], OutputFormat::Html);
    assert!(html.contains("<div class=\"page\">"));
    assert!(html.contains("Hello World"));
    assert!(html.contains("</div>"));
}

#[test]
fn test_format_json() {
    let mut doc = PdfDocument::open(&fixture("with_text.pdf")).unwrap();
    let pages = text::extract_all_text(&mut doc).unwrap();
    let json = format::format_page(&pages[0], OutputFormat::Json);
    assert!(json.contains("\"page_index\": 0"));
    assert!(json.contains("Hello"));
    assert!(json.contains("\"blocks\""));
}

#[test]
fn test_format_markdown() {
    let mut doc = PdfDocument::open(&fixture("with_text.pdf")).unwrap();
    let pages = text::extract_all_text(&mut doc).unwrap();
    let md = format::format_page(&pages[0], OutputFormat::Markdown);
    assert!(md.contains("Hello World"));
}

#[test]
fn test_format_multi_page() {
    let mut doc = PdfDocument::open(&fixture("with_text.pdf")).unwrap();
    let pages = text::extract_all_text(&mut doc).unwrap();
    let html = format::format_pages(&pages, OutputFormat::Html);
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("Page 1"));
}

// ============================================================
// Negative tests
// ============================================================

#[test]
fn test_nonexistent_file() {
    let result = PdfDocument::open(Path::new("does_not_exist.pdf"));
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), JustPdfError::Io(_)));
}

#[test]
fn test_not_a_pdf() {
    let result = PdfDocument::open(&fixture("not_a_pdf.txt"));
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), JustPdfError::NotPdf));
}

#[test]
fn test_empty_file() {
    let result = PdfDocument::open(&fixture("empty.bin"));
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), JustPdfError::NotPdf));
}

#[test]
fn test_truncated_pdf() {
    let result = PdfDocument::open(&fixture("truncated.pdf"));
    assert!(result.is_err());
    // Should fail at xref/trailer stage
}

#[test]
fn test_object_not_found() {
    let mut doc = PdfDocument::open(&fixture("minimal.pdf")).unwrap();
    let result = doc.resolve(&IndirectRef {
        obj_num: 999,
        gen_num: 0,
    });
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        JustPdfError::ObjectNotFound { .. }
    ));
}

#[test]
fn test_corrupted_xref_bad_offset() {
    let mut doc = PdfDocument::open(&fixture("corrupted_xref.pdf")).unwrap();
    // obj 1 has a wrong offset (99999), resolving it should fail
    let result = doc.resolve(&IndirectRef {
        obj_num: 1,
        gen_num: 0,
    });
    assert!(result.is_err());
}
