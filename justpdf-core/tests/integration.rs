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

// ============================================================
// Annotation & Form tests (Phase 5)
// ============================================================

use justpdf_core::annot::{
    self, AnnotationBuilder, AnnotColor, AnnotationType, AnnotationData,
    AnnotationFlags,
};
use justpdf_core::form;
use justpdf_core::writer::document::DocumentBuilder;
use justpdf_core::writer::page::PageBuilder;
use justpdf_core::writer::modify::DocumentModifier;
use justpdf_core::writer::{PdfWriter, serialize_pdf};
use justpdf_core::page::Rect;
use justpdf_core::object::PdfDict;

fn create_simple_pdf() -> Vec<u8> {
    let mut doc = DocumentBuilder::new();
    let font = doc.add_standard_font("Helvetica");
    let mut page = PageBuilder::new(612.0, 792.0);
    page.add_font(&font, "Helvetica");
    page.begin_text();
    page.set_font(&font, 12.0);
    page.move_to(72.0, 720.0);
    page.show_text("Hello World");
    page.end_text();
    doc.add_page(page);
    doc.build().unwrap()
}

#[test]
fn test_no_annotations() {
    let bytes = create_simple_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    let pages = collect_pages(&mut doc).unwrap();
    let annots = annot::get_annotations(&mut doc, &pages[0]).unwrap();
    assert!(annots.is_empty());
}

#[test]
fn test_add_highlight_annotation_roundtrip() {
    let bytes = create_simple_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    let pages = collect_pages(&mut doc).unwrap();
    let page_obj_num = pages[0].page_ref.obj_num;

    let mut modifier = DocumentModifier::from_document(&mut doc).unwrap();
    let rect = Rect { llx: 100.0, lly: 700.0, urx: 300.0, ury: 720.0 };
    let qp = vec![100.0, 720.0, 300.0, 720.0, 100.0, 700.0, 300.0, 700.0];
    let builder = AnnotationBuilder::highlight(rect, qp, AnnotColor::Rgb(1.0, 1.0, 0.0))
        .contents("Test highlight");
    annot::add_annotation(&mut modifier, page_obj_num, builder).unwrap();

    let new_bytes = modifier.build().unwrap();

    // Re-parse and verify
    let mut doc2 = PdfDocument::from_bytes(new_bytes).unwrap();
    let pages2 = collect_pages(&mut doc2).unwrap();
    let annots = annot::get_annotations(&mut doc2, &pages2[0]).unwrap();
    assert_eq!(annots.len(), 1);
    assert_eq!(annots[0].annot_type, AnnotationType::Highlight);
    assert_eq!(annots[0].contents, Some("Test highlight".to_string()));
    assert_eq!(annots[0].color, Some(AnnotColor::Rgb(1.0, 1.0, 0.0)));
}

#[test]
fn test_add_ink_annotation_roundtrip() {
    let bytes = create_simple_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    let pages = collect_pages(&mut doc).unwrap();
    let page_obj_num = pages[0].page_ref.obj_num;

    let mut modifier = DocumentModifier::from_document(&mut doc).unwrap();
    let rect = Rect { llx: 50.0, lly: 50.0, urx: 200.0, ury: 200.0 };
    let ink_list = vec![vec![(60.0, 60.0), (100.0, 150.0), (180.0, 80.0)]];
    let builder = AnnotationBuilder::ink(rect, ink_list.clone())
        .color(AnnotColor::Rgb(1.0, 0.0, 0.0));
    annot::add_annotation(&mut modifier, page_obj_num, builder).unwrap();

    let new_bytes = modifier.build().unwrap();
    let mut doc2 = PdfDocument::from_bytes(new_bytes).unwrap();
    let pages2 = collect_pages(&mut doc2).unwrap();
    let annots = annot::get_annotations(&mut doc2, &pages2[0]).unwrap();
    assert_eq!(annots.len(), 1);
    assert_eq!(annots[0].annot_type, AnnotationType::Ink);
    if let AnnotationData::Ink { ink_list: parsed } = &annots[0].data {
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].len(), 3);
        assert_eq!(parsed[0][0], (60.0, 60.0));
        assert_eq!(parsed[0][2], (180.0, 80.0));
    } else {
        panic!("expected Ink data");
    }
}

#[test]
fn test_add_link_annotation_roundtrip() {
    let bytes = create_simple_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    let pages = collect_pages(&mut doc).unwrap();
    let page_obj_num = pages[0].page_ref.obj_num;

    let mut modifier = DocumentModifier::from_document(&mut doc).unwrap();
    let rect = Rect { llx: 72.0, lly: 700.0, urx: 200.0, ury: 720.0 };
    let builder = AnnotationBuilder::link_uri(rect, "https://example.com");
    annot::add_annotation(&mut modifier, page_obj_num, builder).unwrap();

    let new_bytes = modifier.build().unwrap();
    let mut doc2 = PdfDocument::from_bytes(new_bytes).unwrap();
    let pages2 = collect_pages(&mut doc2).unwrap();
    let annots = annot::get_annotations(&mut doc2, &pages2[0]).unwrap();
    assert_eq!(annots.len(), 1);
    if let AnnotationData::Link { uri, .. } = &annots[0].data {
        assert_eq!(uri.as_deref(), Some("https://example.com"));
    } else {
        panic!("expected Link data");
    }
}

#[test]
fn test_delete_annotation() {
    let bytes = create_simple_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    let pages = collect_pages(&mut doc).unwrap();
    let page_obj_num = pages[0].page_ref.obj_num;

    let mut modifier = DocumentModifier::from_document(&mut doc).unwrap();

    // Add two annotations
    let rect = Rect { llx: 100.0, lly: 700.0, urx: 300.0, ury: 720.0 };
    let builder1 = AnnotationBuilder::text(rect, "Note 1");
    let builder2 = AnnotationBuilder::text(rect, "Note 2");
    annot::add_annotation(&mut modifier, page_obj_num, builder1).unwrap();
    annot::add_annotation(&mut modifier, page_obj_num, builder2).unwrap();

    // Delete first annotation
    annot::delete_annotation(&mut modifier, page_obj_num, 0).unwrap();

    let new_bytes = modifier.build().unwrap();
    let mut doc2 = PdfDocument::from_bytes(new_bytes).unwrap();
    let pages2 = collect_pages(&mut doc2).unwrap();
    let annots = annot::get_annotations(&mut doc2, &pages2[0]).unwrap();
    assert_eq!(annots.len(), 1);
    assert_eq!(annots[0].contents, Some("Note 2".to_string()));
}

#[test]
fn test_delete_annotation_out_of_range() {
    let bytes = create_simple_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    let pages = collect_pages(&mut doc).unwrap();
    let page_obj_num = pages[0].page_ref.obj_num;

    let mut modifier = DocumentModifier::from_document(&mut doc).unwrap();
    let result = annot::delete_annotation(&mut modifier, page_obj_num, 0);
    assert!(result.is_err());
}

#[test]
fn test_line_annotation_properties() {
    let bytes = create_simple_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    let pages = collect_pages(&mut doc).unwrap();
    let page_obj_num = pages[0].page_ref.obj_num;

    let mut modifier = DocumentModifier::from_document(&mut doc).unwrap();
    let builder = AnnotationBuilder::line((100.0, 100.0), (300.0, 300.0))
        .line_endings(
            annot::LineEndingStyle::OpenArrow,
            annot::LineEndingStyle::ClosedArrow,
        )
        .color(AnnotColor::Rgb(0.0, 0.0, 1.0));
    annot::add_annotation(&mut modifier, page_obj_num, builder).unwrap();

    let new_bytes = modifier.build().unwrap();
    let mut doc2 = PdfDocument::from_bytes(new_bytes).unwrap();
    let pages2 = collect_pages(&mut doc2).unwrap();
    let annots = annot::get_annotations(&mut doc2, &pages2[0]).unwrap();
    assert_eq!(annots.len(), 1);
    if let AnnotationData::Line { start, end, line_endings, .. } = &annots[0].data {
        assert_eq!(*start, (100.0, 100.0));
        assert_eq!(*end, (300.0, 300.0));
        assert_eq!(line_endings.0, annot::LineEndingStyle::OpenArrow);
        assert_eq!(line_endings.1, annot::LineEndingStyle::ClosedArrow);
    } else {
        panic!("expected Line data");
    }
}

#[test]
fn test_annotation_flags_roundtrip() {
    let bytes = create_simple_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    let pages = collect_pages(&mut doc).unwrap();
    let page_obj_num = pages[0].page_ref.obj_num;

    let mut modifier = DocumentModifier::from_document(&mut doc).unwrap();
    let rect = Rect { llx: 100.0, lly: 600.0, urx: 250.0, ury: 650.0 };
    let builder = AnnotationBuilder::stamp(rect, "Approved")
        .flags(AnnotationFlags(AnnotationFlags::PRINT | AnnotationFlags::NO_ZOOM));
    annot::add_annotation(&mut modifier, page_obj_num, builder).unwrap();

    let new_bytes = modifier.build().unwrap();
    let mut doc2 = PdfDocument::from_bytes(new_bytes).unwrap();
    let pages2 = collect_pages(&mut doc2).unwrap();
    let annots = annot::get_annotations(&mut doc2, &pages2[0]).unwrap();
    assert_eq!(annots.len(), 1);
    assert!(annots[0].flags.has(AnnotationFlags::PRINT));
    assert!(annots[0].flags.has(AnnotationFlags::NO_ZOOM));
    assert!(!annots[0].flags.has(AnnotationFlags::HIDDEN));
}

// ============================================================
// Form tests
// ============================================================

/// Create a PDF with AcroForm fields using low-level PdfWriter.
fn create_acroform_pdf() -> Vec<u8> {
    let mut w = PdfWriter::new();

    // 1. Font (Helvetica)
    let mut font_dict = PdfDict::new();
    font_dict.insert(b"Type".to_vec(), PdfObject::Name(b"Font".to_vec()));
    font_dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Type1".to_vec()));
    font_dict.insert(b"BaseFont".to_vec(), PdfObject::Name(b"Helvetica".to_vec()));
    let font_ref = w.add_object(PdfObject::Dict(font_dict));

    // 2. Resources dict
    let mut font_map = PdfDict::new();
    font_map.insert(b"Helv".to_vec(), PdfObject::Reference(font_ref.clone()));
    let mut resources = PdfDict::new();
    resources.insert(b"Font".to_vec(), PdfObject::Dict(font_map));
    let resources_ref = w.add_object(PdfObject::Dict(resources));

    // 3. Text field
    let mut text_field = PdfDict::new();
    text_field.insert(b"Type".to_vec(), PdfObject::Name(b"Annot".to_vec()));
    text_field.insert(b"Subtype".to_vec(), PdfObject::Name(b"Widget".to_vec()));
    text_field.insert(b"FT".to_vec(), PdfObject::Name(b"Tx".to_vec()));
    text_field.insert(b"T".to_vec(), PdfObject::String(b"name".to_vec()));
    text_field.insert(b"V".to_vec(), PdfObject::String(b"John".to_vec()));
    text_field.insert(b"DA".to_vec(), PdfObject::String(b"/Helv 10 Tf 0 g".to_vec()));
    text_field.insert(
        b"Rect".to_vec(),
        PdfObject::Array(vec![
            PdfObject::Integer(100), PdfObject::Integer(700),
            PdfObject::Integer(300), PdfObject::Integer(720),
        ]),
    );
    let text_ref = w.add_object(PdfObject::Dict(text_field));

    // 4. Checkbox field
    let mut cb_field = PdfDict::new();
    cb_field.insert(b"Type".to_vec(), PdfObject::Name(b"Annot".to_vec()));
    cb_field.insert(b"Subtype".to_vec(), PdfObject::Name(b"Widget".to_vec()));
    cb_field.insert(b"FT".to_vec(), PdfObject::Name(b"Btn".to_vec()));
    cb_field.insert(b"T".to_vec(), PdfObject::String(b"agree".to_vec()));
    cb_field.insert(b"V".to_vec(), PdfObject::Name(b"Off".to_vec()));
    cb_field.insert(b"AS".to_vec(), PdfObject::Name(b"Off".to_vec()));
    cb_field.insert(
        b"Rect".to_vec(),
        PdfObject::Array(vec![
            PdfObject::Integer(100), PdfObject::Integer(660),
            PdfObject::Integer(114), PdfObject::Integer(674),
        ]),
    );
    let cb_ref = w.add_object(PdfObject::Dict(cb_field));

    // 5. ComboBox field
    let mut combo_field = PdfDict::new();
    combo_field.insert(b"Type".to_vec(), PdfObject::Name(b"Annot".to_vec()));
    combo_field.insert(b"Subtype".to_vec(), PdfObject::Name(b"Widget".to_vec()));
    combo_field.insert(b"FT".to_vec(), PdfObject::Name(b"Ch".to_vec()));
    combo_field.insert(b"Ff".to_vec(), PdfObject::Integer(1 << 17)); // Combo flag
    combo_field.insert(b"T".to_vec(), PdfObject::String(b"country".to_vec()));
    combo_field.insert(b"V".to_vec(), PdfObject::String(b"Korea".to_vec()));
    combo_field.insert(
        b"Opt".to_vec(),
        PdfObject::Array(vec![
            PdfObject::String(b"Korea".to_vec()),
            PdfObject::String(b"Japan".to_vec()),
            PdfObject::String(b"China".to_vec()),
        ]),
    );
    combo_field.insert(
        b"Rect".to_vec(),
        PdfObject::Array(vec![
            PdfObject::Integer(100), PdfObject::Integer(620),
            PdfObject::Integer(300), PdfObject::Integer(640),
        ]),
    );
    let combo_ref = w.add_object(PdfObject::Dict(combo_field));

    // 6. ReadOnly text field
    let mut ro_field = PdfDict::new();
    ro_field.insert(b"Type".to_vec(), PdfObject::Name(b"Annot".to_vec()));
    ro_field.insert(b"Subtype".to_vec(), PdfObject::Name(b"Widget".to_vec()));
    ro_field.insert(b"FT".to_vec(), PdfObject::Name(b"Tx".to_vec()));
    ro_field.insert(b"T".to_vec(), PdfObject::String(b"id".to_vec()));
    ro_field.insert(b"V".to_vec(), PdfObject::String(b"12345".to_vec()));
    ro_field.insert(b"Ff".to_vec(), PdfObject::Integer(1)); // ReadOnly
    ro_field.insert(
        b"Rect".to_vec(),
        PdfObject::Array(vec![
            PdfObject::Integer(100), PdfObject::Integer(580),
            PdfObject::Integer(300), PdfObject::Integer(600),
        ]),
    );
    let ro_ref = w.add_object(PdfObject::Dict(ro_field));

    // 7. AcroForm
    let mut acroform = PdfDict::new();
    acroform.insert(
        b"Fields".to_vec(),
        PdfObject::Array(vec![
            PdfObject::Reference(text_ref.clone()),
            PdfObject::Reference(cb_ref.clone()),
            PdfObject::Reference(combo_ref.clone()),
            PdfObject::Reference(ro_ref.clone()),
        ]),
    );
    acroform.insert(
        b"DR".to_vec(),
        PdfObject::Reference(resources_ref.clone()),
    );
    acroform.insert(
        b"DA".to_vec(),
        PdfObject::String(b"/Helv 10 Tf 0 g".to_vec()),
    );

    // 8. Page (with widget annotations)
    let pages_num = w.alloc_object_num();
    let pages_ref = IndirectRef { obj_num: pages_num, gen_num: 0 };

    let mut page_dict = PdfDict::new();
    page_dict.insert(b"Type".to_vec(), PdfObject::Name(b"Page".to_vec()));
    page_dict.insert(b"Parent".to_vec(), PdfObject::Reference(pages_ref.clone()));
    page_dict.insert(
        b"MediaBox".to_vec(),
        PdfObject::Array(vec![
            PdfObject::Integer(0), PdfObject::Integer(0),
            PdfObject::Integer(612), PdfObject::Integer(792),
        ]),
    );
    page_dict.insert(b"Resources".to_vec(), PdfObject::Reference(resources_ref));
    page_dict.insert(
        b"Annots".to_vec(),
        PdfObject::Array(vec![
            PdfObject::Reference(text_ref),
            PdfObject::Reference(cb_ref),
            PdfObject::Reference(combo_ref),
            PdfObject::Reference(ro_ref),
        ]),
    );
    let page_ref = w.add_object(PdfObject::Dict(page_dict));

    // 9. Pages
    let mut pages_dict = PdfDict::new();
    pages_dict.insert(b"Type".to_vec(), PdfObject::Name(b"Pages".to_vec()));
    pages_dict.insert(
        b"Kids".to_vec(),
        PdfObject::Array(vec![PdfObject::Reference(page_ref)]),
    );
    pages_dict.insert(b"Count".to_vec(), PdfObject::Integer(1));
    w.set_object(pages_num, PdfObject::Dict(pages_dict));

    // 10. Catalog
    let mut catalog = PdfDict::new();
    catalog.insert(b"Type".to_vec(), PdfObject::Name(b"Catalog".to_vec()));
    catalog.insert(b"Pages".to_vec(), PdfObject::Reference(pages_ref));
    catalog.insert(b"AcroForm".to_vec(), PdfObject::Dict(acroform));
    let catalog_ref = w.add_object(PdfObject::Dict(catalog));

    serialize_pdf(&w.objects, (1, 7), &catalog_ref, None).unwrap()
}

#[test]
fn test_acroform_parse() {
    let bytes = create_acroform_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    let acroform = form::parse_acroform(&mut doc).unwrap().unwrap();

    assert_eq!(acroform.fields.len(), 4);

    let text_field = acroform.fields.iter().find(|f| f.name == "name").unwrap();
    assert_eq!(text_field.field_type, form::FieldType::Text);
    assert_eq!(text_field.value_as_string(), Some("John".to_string()));

    let cb_field = acroform.fields.iter().find(|f| f.name == "agree").unwrap();
    assert_eq!(cb_field.field_type, form::FieldType::Checkbox);
    assert!(!cb_field.is_checked());

    let combo = acroform.fields.iter().find(|f| f.name == "country").unwrap();
    assert_eq!(combo.field_type, form::FieldType::ComboBox);
    assert_eq!(combo.value_as_string(), Some("Korea".to_string()));
    assert_eq!(combo.options, vec!["Korea", "Japan", "China"]);

    let ro_field = acroform.fields.iter().find(|f| f.name == "id").unwrap();
    assert!(ro_field.flags.is_read_only());
}

#[test]
fn test_acroform_no_form() {
    let bytes = create_simple_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    let acroform = form::parse_acroform(&mut doc).unwrap();
    assert!(acroform.is_none());
}

#[test]
fn test_set_text_field_value_roundtrip() {
    let bytes = create_acroform_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    let acroform = form::parse_acroform(&mut doc).unwrap().unwrap();

    let mut modifier = DocumentModifier::from_document(&mut doc).unwrap();
    form::set_field_value(
        &mut modifier,
        &acroform,
        "name",
        PdfObject::String(b"Jane".to_vec()),
    )
    .unwrap();

    let new_bytes = modifier.build().unwrap();
    let mut doc2 = PdfDocument::from_bytes(new_bytes).unwrap();
    let acroform2 = form::parse_acroform(&mut doc2).unwrap().unwrap();
    let field = acroform2.fields.iter().find(|f| f.name == "name").unwrap();
    assert_eq!(field.value_as_string(), Some("Jane".to_string()));
}

#[test]
fn test_checkbox_toggle_roundtrip() {
    let bytes = create_acroform_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    let acroform = form::parse_acroform(&mut doc).unwrap().unwrap();

    // Initially Off
    let cb = acroform.fields.iter().find(|f| f.name == "agree").unwrap();
    assert!(!cb.is_checked());

    // Toggle on
    let mut modifier = DocumentModifier::from_document(&mut doc).unwrap();
    form::toggle_checkbox(&mut modifier, &acroform, "agree").unwrap();

    let new_bytes = modifier.build().unwrap();
    let mut doc2 = PdfDocument::from_bytes(new_bytes).unwrap();
    let acroform2 = form::parse_acroform(&mut doc2).unwrap().unwrap();
    let cb2 = acroform2.fields.iter().find(|f| f.name == "agree").unwrap();
    assert!(cb2.is_checked());
}

#[test]
fn test_combobox_change_roundtrip() {
    let bytes = create_acroform_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    let acroform = form::parse_acroform(&mut doc).unwrap().unwrap();

    let mut modifier = DocumentModifier::from_document(&mut doc).unwrap();
    form::set_field_value(
        &mut modifier,
        &acroform,
        "country",
        PdfObject::String(b"Japan".to_vec()),
    )
    .unwrap();

    let new_bytes = modifier.build().unwrap();
    let mut doc2 = PdfDocument::from_bytes(new_bytes).unwrap();
    let acroform2 = form::parse_acroform(&mut doc2).unwrap().unwrap();
    let combo = acroform2.fields.iter().find(|f| f.name == "country").unwrap();
    assert_eq!(combo.value_as_string(), Some("Japan".to_string()));
}

#[test]
fn test_readonly_field_error() {
    let bytes = create_acroform_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    let acroform = form::parse_acroform(&mut doc).unwrap().unwrap();

    let mut modifier = DocumentModifier::from_document(&mut doc).unwrap();
    let result = form::set_field_value(
        &mut modifier,
        &acroform,
        "id",
        PdfObject::String(b"99999".to_vec()),
    );
    assert!(result.is_err());
}

#[test]
fn test_flatten_form() {
    let bytes = create_acroform_pdf();
    let mut doc = PdfDocument::from_bytes(bytes.clone()).unwrap();

    let mut modifier = DocumentModifier::from_document(&mut doc).unwrap();
    // Re-open doc for flatten (needs separate borrow)
    let mut doc_for_flatten = PdfDocument::from_bytes(bytes).unwrap();
    form::flatten_form(&mut modifier, &mut doc_for_flatten).unwrap();

    let new_bytes = modifier.build().unwrap();
    let mut doc2 = PdfDocument::from_bytes(new_bytes).unwrap();

    // AcroForm should be removed
    let acroform2 = form::parse_acroform(&mut doc2).unwrap();
    assert!(acroform2.is_none());
}

#[test]
fn test_redaction_apply() {
    // Create a PDF with text, add redact annotation, apply redaction
    let bytes = create_simple_pdf(); // has "Hello World" at (72, 720)
    let mut doc = PdfDocument::from_bytes(bytes.clone()).unwrap();
    let pages = collect_pages(&mut doc).unwrap();
    let page_obj_num = pages[0].page_ref.obj_num;

    // Add a Redact annotation covering the text area
    let mut modifier = DocumentModifier::from_document(&mut doc).unwrap();
    let rect = Rect { llx: 50.0, lly: 710.0, urx: 200.0, ury: 730.0 };
    let builder = AnnotationBuilder::redact(rect)
        .interior_color(AnnotColor::Rgb(0.0, 0.0, 0.0));
    annot::add_annotation(&mut modifier, page_obj_num, builder).unwrap();
    let with_redact = modifier.build().unwrap();

    // Now apply redaction
    let mut doc2 = PdfDocument::from_bytes(with_redact.clone()).unwrap();
    let mut modifier2 = DocumentModifier::from_document(&mut doc2).unwrap();
    let mut doc_for_apply = PdfDocument::from_bytes(with_redact).unwrap();
    annot::redact::apply_redactions(&mut modifier2, &mut doc_for_apply, 0).unwrap();

    let result_bytes = modifier2.build().unwrap();

    // Verify: redact annotation should be removed
    let mut doc3 = PdfDocument::from_bytes(result_bytes).unwrap();
    let pages3 = collect_pages(&mut doc3).unwrap();
    let annots = annot::get_annotations(&mut doc3, &pages3[0]).unwrap();
    // No redact annotations remaining
    assert!(
        annots.iter().all(|a| a.annot_type != AnnotationType::Redact),
        "redact annotations should be removed"
    );
}

// ============================================================
// Encryption tests (Phase 6)
// ============================================================

use justpdf_core::crypto;

#[test]
fn test_encrypt_rc4_128_roundtrip() {
    // Create an encrypted PDF using RC4-128
    let mut builder = justpdf_core::writer::document::DocumentBuilder::new();
    let font_name = builder.add_standard_font("Helvetica");

    let mut page = justpdf_core::writer::page::PageBuilder::new(612.0, 792.0);
    page.add_font(&font_name, "Helvetica");
    page.begin_text();
    page.set_font(&font_name, 24.0);
    page.move_to(72.0, 720.0);
    page.show_text("Secret RC4 Content");
    page.end_text();
    builder.add_page(page);
    builder.set_title("Encrypted RC4 Doc");

    builder.set_encryption(crypto::EncryptionConfig {
        user_password: b"user123".to_vec(),
        owner_password: b"owner456".to_vec(),
        permissions: crypto::Permissions::allow_all(),
        method: crypto::EncryptionMethod::RC4_128,
        encrypt_metadata: true,
    });

    let bytes = builder.build().unwrap();

    // Verify it's a valid PDF
    assert!(bytes.starts_with(b"%PDF-1.7"));

    // The raw bytes should NOT contain plaintext "Secret RC4 Content"
    let text = String::from_utf8_lossy(&bytes);
    assert!(!text.contains("Secret RC4 Content"));

    // Open and authenticate with user password
    let mut doc = PdfDocument::from_bytes(bytes.clone()).unwrap();
    assert!(doc.is_encrypted());

    // Try wrong password first
    assert!(doc.authenticate(b"wrong").is_err());

    // Authenticate with correct user password
    doc.authenticate(b"user123").unwrap();
    assert!(doc.is_authenticated());

    // Verify we can parse pages
    let pages = collect_pages(&mut doc).unwrap();
    assert_eq!(pages.len(), 1);
}

#[test]
fn test_encrypt_aes128_roundtrip() {
    let mut builder = justpdf_core::writer::document::DocumentBuilder::new();
    let font_name = builder.add_standard_font("Courier");

    let mut page = justpdf_core::writer::page::PageBuilder::new(612.0, 792.0);
    page.add_font(&font_name, "Courier");
    page.begin_text();
    page.set_font(&font_name, 12.0);
    page.move_to(72.0, 700.0);
    page.show_text("AES-128 Encrypted Text");
    page.end_text();
    builder.add_page(page);

    builder.set_encryption(crypto::EncryptionConfig {
        user_password: b"aes128pass".to_vec(),
        owner_password: b"aes128owner".to_vec(),
        permissions: crypto::Permissions::allow_all(),
        method: crypto::EncryptionMethod::AES128,
        encrypt_metadata: true,
    });

    let bytes = builder.build().unwrap();

    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    assert!(doc.is_encrypted());

    doc.authenticate(b"aes128pass").unwrap();
    let pages = collect_pages(&mut doc).unwrap();
    assert_eq!(pages.len(), 1);
}

#[test]
fn test_encrypt_aes256_roundtrip() {
    let mut builder = justpdf_core::writer::document::DocumentBuilder::new();
    let font_name = builder.add_standard_font("Helvetica");

    let mut page = justpdf_core::writer::page::PageBuilder::new(612.0, 792.0);
    page.add_font(&font_name, "Helvetica");
    page.begin_text();
    page.set_font(&font_name, 18.0);
    page.move_to(72.0, 700.0);
    page.show_text("AES-256 Top Secret");
    page.end_text();
    builder.add_page(page);

    builder.set_encryption(crypto::EncryptionConfig {
        user_password: b"aes256user".to_vec(),
        owner_password: b"aes256owner".to_vec(),
        permissions: crypto::Permissions::allow_all(),
        method: crypto::EncryptionMethod::AES256,
        encrypt_metadata: true,
    });

    let bytes = builder.build().unwrap();

    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    assert!(doc.is_encrypted());

    // Authenticate with owner password
    doc.authenticate(b"aes256owner").unwrap();
    assert!(doc.is_authenticated());

    let pages = collect_pages(&mut doc).unwrap();
    assert_eq!(pages.len(), 1);
}

#[test]
fn test_encrypt_empty_user_password() {
    // Many real PDFs use empty user password (open access but restricted permissions)
    let mut builder = justpdf_core::writer::document::DocumentBuilder::new();
    let page = justpdf_core::writer::page::PageBuilder::new(612.0, 792.0);
    builder.add_page(page);

    builder.set_encryption(crypto::EncryptionConfig {
        user_password: vec![],
        owner_password: b"admin".to_vec(),
        permissions: crypto::Permissions::new(0xFFFFF0C4u32 as i32), // only print
        method: crypto::EncryptionMethod::RC4_128,
        encrypt_metadata: true,
    });

    let bytes = builder.build().unwrap();

    // Should auto-authenticate with empty password
    let doc = PdfDocument::from_bytes(bytes).unwrap();
    assert!(doc.is_encrypted());
    assert!(doc.is_authenticated()); // empty password auto-authenticated

    // Check permissions
    let perms = doc.permissions().unwrap();
    assert!(perms.can_print());
    assert!(!perms.can_copy());
    assert!(!perms.can_modify());
}

#[test]
fn test_encrypt_incorrect_password() {
    let mut builder = justpdf_core::writer::document::DocumentBuilder::new();
    let page = justpdf_core::writer::page::PageBuilder::new(612.0, 792.0);
    builder.add_page(page);

    builder.set_encryption(crypto::EncryptionConfig {
        user_password: b"secret".to_vec(),
        owner_password: b"secret".to_vec(),
        permissions: crypto::Permissions::allow_all(),
        method: crypto::EncryptionMethod::AES128,
        encrypt_metadata: true,
    });

    let bytes = builder.build().unwrap();

    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    assert!(doc.is_encrypted());
    assert!(!doc.is_authenticated()); // non-empty password

    // Wrong password
    let err = doc.authenticate(b"wrong").unwrap_err();
    assert!(matches!(err, JustPdfError::IncorrectPassword));

    // Trying to resolve objects without auth should fail
    let cat_ref = doc.catalog_ref().unwrap().clone();
    let err = doc.resolve(&cat_ref).unwrap_err();
    assert!(matches!(err, JustPdfError::EncryptedDocument));

    // Correct password
    doc.authenticate(b"secret").unwrap();
    assert!(doc.is_authenticated());
}

#[test]
fn test_unencrypted_pdf_not_encrypted() {
    let bytes = create_simple_pdf();
    let doc = PdfDocument::from_bytes(bytes).unwrap();
    assert!(!doc.is_encrypted());
    assert!(doc.is_authenticated());
    assert!(doc.permissions().is_none());
}

#[test]
fn test_encrypt_permissions_flags() {
    let perms = crypto::Permissions::allow_all();
    assert!(perms.can_print());
    assert!(perms.can_modify());
    assert!(perms.can_copy());
    assert!(perms.can_annotate());
    assert!(perms.can_fill_forms());
    assert!(perms.can_assemble());
    assert!(perms.can_print_high_quality());

    // No permissions (only required bits set)
    let no_perms = crypto::Permissions::new(0xFFFFF0C0u32 as i32);
    assert!(!no_perms.can_print());
    assert!(!no_perms.can_modify());
    assert!(!no_perms.can_copy());
    assert!(!no_perms.can_annotate());
}

// ============================================================
// Phase 8 Memory Optimization tests (section 8.3)
// ============================================================

use justpdf_core::page::{self as page_mod, get_page};
use justpdf_core::stream::decode_stream_cow;
use std::borrow::Cow;

/// Create a synthetic multi-page PDF with the given number of pages.
fn create_multi_page_pdf(num_pages: usize) -> Vec<u8> {
    let mut w = PdfWriter::new();

    // Font
    let mut font_dict = PdfDict::new();
    font_dict.insert(b"Type".to_vec(), PdfObject::Name(b"Font".to_vec()));
    font_dict.insert(b"Subtype".to_vec(), PdfObject::Name(b"Type1".to_vec()));
    font_dict.insert(b"BaseFont".to_vec(), PdfObject::Name(b"Helvetica".to_vec()));
    let font_ref = w.add_object(PdfObject::Dict(font_dict));

    // Resources
    let mut font_map = PdfDict::new();
    font_map.insert(b"F1".to_vec(), PdfObject::Reference(font_ref.clone()));
    let mut resources = PdfDict::new();
    resources.insert(b"Font".to_vec(), PdfObject::Dict(font_map));
    let resources_ref = w.add_object(PdfObject::Dict(resources));

    // Pre-allocate Pages node
    let pages_num = w.alloc_object_num();
    let pages_ref = IndirectRef { obj_num: pages_num, gen_num: 0 };

    // Create page objects
    let mut page_refs = Vec::with_capacity(num_pages);
    for i in 0..num_pages {
        let content = format!("BT /F1 12 Tf 72 720 Td (Page {}) Tj ET", i + 1);
        let content_bytes = content.into_bytes();

        // Content stream (uncompressed for speed)
        let mut stream_dict = PdfDict::new();
        stream_dict.insert(
            b"Length".to_vec(),
            PdfObject::Integer(content_bytes.len() as i64),
        );
        let stream_obj = PdfObject::Stream { dict: stream_dict, data: content_bytes };
        let content_ref = w.add_object(stream_obj);

        let mut page_dict = PdfDict::new();
        page_dict.insert(b"Type".to_vec(), PdfObject::Name(b"Page".to_vec()));
        page_dict.insert(b"Parent".to_vec(), PdfObject::Reference(pages_ref.clone()));
        page_dict.insert(
            b"MediaBox".to_vec(),
            PdfObject::Array(vec![
                PdfObject::Integer(0),
                PdfObject::Integer(0),
                PdfObject::Integer(612),
                PdfObject::Integer(792),
            ]),
        );
        page_dict.insert(b"Resources".to_vec(), PdfObject::Reference(resources_ref.clone()));
        page_dict.insert(b"Contents".to_vec(), PdfObject::Reference(content_ref));
        let page_ref = w.add_object(PdfObject::Dict(page_dict));
        page_refs.push(PdfObject::Reference(page_ref));
    }

    // Pages node
    let mut pages_dict = PdfDict::new();
    pages_dict.insert(b"Type".to_vec(), PdfObject::Name(b"Pages".to_vec()));
    pages_dict.insert(b"Kids".to_vec(), PdfObject::Array(page_refs));
    pages_dict.insert(b"Count".to_vec(), PdfObject::Integer(num_pages as i64));
    w.set_object(pages_num, PdfObject::Dict(pages_dict));

    // Catalog
    let mut catalog = PdfDict::new();
    catalog.insert(b"Type".to_vec(), PdfObject::Name(b"Catalog".to_vec()));
    catalog.insert(b"Pages".to_vec(), PdfObject::Reference(pages_ref));
    let catalog_ref = w.add_object(PdfObject::Dict(catalog));

    serialize_pdf(&w.objects, (1, 7), &catalog_ref, None).unwrap()
}

// --- page_count tests ---

#[test]
fn test_page_count_single() {
    let bytes = create_simple_pdf();
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    assert_eq!(page_mod::page_count(&mut doc).unwrap(), 1);
}

#[test]
fn test_page_count_multi() {
    let bytes = create_multi_page_pdf(25);
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    assert_eq!(page_mod::page_count(&mut doc).unwrap(), 25);
}

#[test]
fn test_page_count_large() {
    let bytes = create_multi_page_pdf(1000);
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    assert_eq!(page_mod::page_count(&mut doc).unwrap(), 1000);
}

// --- get_page tests ---

#[test]
fn test_get_page_first() {
    let bytes = create_multi_page_pdf(10);
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    let page = get_page(&mut doc, 0).unwrap();
    assert_eq!(page.index, 0);
    assert_eq!(page.media_box.width(), 612.0);
    assert_eq!(page.media_box.height(), 792.0);
}

#[test]
fn test_get_page_last() {
    let bytes = create_multi_page_pdf(10);
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    let page = get_page(&mut doc, 9).unwrap();
    assert_eq!(page.index, 9);
}

#[test]
fn test_get_page_middle() {
    let bytes = create_multi_page_pdf(50);
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    let page = get_page(&mut doc, 25).unwrap();
    assert_eq!(page.index, 25);
}

#[test]
fn test_get_page_out_of_range() {
    let bytes = create_multi_page_pdf(5);
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    let result = get_page(&mut doc, 5);
    assert!(result.is_err());
}

#[test]
fn test_get_page_out_of_range_large_index() {
    let bytes = create_multi_page_pdf(3);
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    let result = get_page(&mut doc, 999);
    assert!(result.is_err());
}

#[test]
fn test_get_page_matches_collect_pages() {
    let bytes = create_multi_page_pdf(20);
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();
    let all_pages = collect_pages(&mut doc).unwrap();

    // Verify get_page returns the same info for each page
    for (i, expected) in all_pages.iter().enumerate() {
        let page = get_page(&mut doc, i).unwrap();
        assert_eq!(page.index, expected.index);
        assert_eq!(page.page_ref, expected.page_ref);
        assert_eq!(page.media_box, expected.media_box);
        assert_eq!(page.rotate, expected.rotate);
    }
}

// --- Large PDF support test ---

#[test]
fn test_large_pdf_1000_pages() {
    let bytes = create_multi_page_pdf(1000);

    // Parse the document
    let mut doc = PdfDocument::from_bytes(bytes).unwrap();

    // page_count should be correct without resolving all pages
    assert_eq!(page_mod::page_count(&mut doc).unwrap(), 1000);

    // Single page access should work
    let first = get_page(&mut doc, 0).unwrap();
    assert_eq!(first.index, 0);

    let middle = get_page(&mut doc, 500).unwrap();
    assert_eq!(middle.index, 500);

    let last = get_page(&mut doc, 999).unwrap();
    assert_eq!(last.index, 999);

    // Out-of-range should fail
    assert!(get_page(&mut doc, 1000).is_err());
}

// --- decode_stream_cow tests (integration) ---

#[test]
fn test_decode_stream_cow_no_filter_borrowed() {
    let dict = PdfDict::new();
    let data = b"plain text content";
    let result = decode_stream_cow(data, &dict).unwrap();
    assert!(matches!(result, Cow::Borrowed(_)));
    assert_eq!(&*result, b"plain text content");
}

#[test]
fn test_decode_stream_cow_dct_borrowed() {
    let mut dict = PdfDict::new();
    dict.insert(
        b"Filter".to_vec(),
        PdfObject::Name(b"DCTDecode".to_vec()),
    );
    let data = b"\xFF\xD8\xFF\xE0fake jpeg bytes";
    let result = decode_stream_cow(data, &dict).unwrap();
    assert!(matches!(result, Cow::Borrowed(_)));
}

#[test]
fn test_decode_stream_cow_flate_owned() {
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;

    let original = b"Compressed content for cow test";
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(original).unwrap();
    let compressed = encoder.finish().unwrap();

    let mut dict = PdfDict::new();
    dict.insert(
        b"Filter".to_vec(),
        PdfObject::Name(b"FlateDecode".to_vec()),
    );

    let result = decode_stream_cow(&compressed, &dict).unwrap();
    assert!(matches!(result, Cow::Owned(_)));
    assert_eq!(&*result, original.as_slice());
}
