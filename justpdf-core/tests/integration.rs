use std::path::Path;

use justpdf_core::{IndirectRef, JustPdfError, PdfDocument, PdfObject};

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
