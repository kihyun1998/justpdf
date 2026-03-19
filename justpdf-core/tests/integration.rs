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
