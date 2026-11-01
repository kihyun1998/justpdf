//! `sign_pdf` links the signature into the document: a field in
//! `/AcroForm /Fields` and a widget in the chosen page's `/Annots`, with
//! `/M` and `/ContactInfo` in the signature dictionary, so `detect_signatures`
//! finds it and `verify_signature` accepts it (#37).

use justpdf_core::PdfDocument;
use justpdf_core::error::JustPdfError;
use justpdf_core::object::{IndirectRef, PdfObject};
use justpdf_core::sign::{SigningOptions, detect_signatures, sign_pdf, verify_signature};

const KEY: &[u8] = include_bytes!("fixtures/sign_test_key.der");
const CERT: &[u8] = include_bytes!("fixtures/sign_test_cert.der");

/// A PDF whose objects are `objects`, each `(number, body)`, with one xref
/// subsection per object and `/Root 1 0 R`.
fn pdf(objects: &[(u32, &str)]) -> Vec<u8> {
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (num, body) in objects {
        offsets.push((*num, out.len()));
        out.extend_from_slice(format!("{num} 0 obj\n{body}\nendobj\n").as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(b"xref\n0 1\n0000000000 65535 f \n");
    for (num, offset) in &offsets {
        out.extend_from_slice(format!("{num} 1\n{offset:010} 00000 n \n").as_bytes());
    }
    let size = objects.iter().map(|(n, _)| n).max().unwrap() + 1;
    out.extend_from_slice(
        format!("trailer\n<< /Size {size} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    out
}

const ONE_PAGE: &[(u32, &str)] = &[
    (1, "<< /Type /Catalog /Pages 2 0 R >>"),
    (2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
    (3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>"),
];

fn sign(source: &[u8], options: &SigningOptions) -> Vec<u8> {
    sign_pdf(source, KEY, &[CERT], options).unwrap()
}

fn obj(num: u32) -> IndirectRef {
    IndirectRef {
        obj_num: num,
        gen_num: 0,
    }
}

fn dict(doc: &PdfDocument, num: u32) -> justpdf_core::object::PdfDict {
    doc.resolve(&obj(num)).unwrap().as_dict().unwrap().clone()
}

/// The references in the array at `key` of `d`, resolving an indirect array.
fn refs(doc: &PdfDocument, d: &justpdf_core::object::PdfDict, key: &[u8]) -> Vec<IndirectRef> {
    let array = match d.get(key) {
        Some(PdfObject::Reference(r)) => doc.resolve(r).unwrap(),
        Some(other) => other.clone(),
        None => return Vec::new(),
    };
    match array {
        PdfObject::Array(items) => items
            .iter()
            .map(|item| match item {
                PdfObject::Reference(r) => r.clone(),
                other => panic!("not a reference: {other:?}"),
            })
            .collect(),
        other => panic!("not an array: {other:?}"),
    }
}

/// The catalog's `/AcroForm`, resolved.
fn acroform(doc: &PdfDocument) -> justpdf_core::object::PdfDict {
    let catalog = dict(doc, doc.catalog_ref().unwrap().obj_num);
    match catalog.get(b"AcroForm").unwrap() {
        PdfObject::Reference(r) => doc.resolve(r).unwrap().as_dict().unwrap().clone(),
        PdfObject::Dict(d) => d.clone(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_signature_is_detected_with_its_entries_and_verifies() {
    let options = SigningOptions {
        signer_name: Some("Signer".into()),
        reason: Some("Approval".into()),
        location: Some("Seoul".into()),
        contact_info: Some("signer@example.com".into()),
        ..Default::default()
    };
    let signed = sign(&pdf(ONE_PAGE), &options);
    let doc = PdfDocument::from_bytes(signed.clone()).unwrap();
    let sigs = detect_signatures(&doc).unwrap();
    assert_eq!(sigs.len(), 1);
    let sig = &sigs[0];
    assert_eq!(sig.field_name, "Signature1");
    assert_eq!(sig.signer_name.as_deref(), Some("Signer"));
    assert_eq!(sig.reason.as_deref(), Some("Approval"));
    assert_eq!(sig.location.as_deref(), Some("Seoul"));
    assert_eq!(sig.contact_info.as_deref(), Some("signer@example.com"));
    let m = sig.signing_time.as_deref().unwrap();
    assert!(
        m.starts_with("D:20") && m.ends_with('Z') && m.len() == 17,
        "{m}"
    );

    let result = verify_signature(&signed, sig).unwrap();
    assert!(result.digest_valid, "{:?}", result.validity);
    assert!(result.signature_valid, "{:?}", result.validity);

    let acroform = acroform(&doc);
    assert_eq!(acroform.get_i64(b"SigFlags"), Some(3));
    let page_annots = refs(&doc, &dict(&doc, 3), b"Annots");
    assert_eq!(page_annots, refs(&doc, &acroform, b"Fields"));
    let widget = doc.resolve(&page_annots[0]).unwrap();
    assert_eq!(
        widget.as_dict().unwrap().get(b"P"),
        Some(&PdfObject::Reference(obj(3)))
    );
}

#[test]
fn existing_fields_are_kept_and_the_name_is_new() {
    for (catalog, acroform_obj) in [
        // A direct /AcroForm with a direct /Fields.
        (
            "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [4 0 R] /SigFlags 0 >> >>",
            None,
        ),
        // An indirect /AcroForm with an indirect /Fields.
        (
            "<< /Type /Catalog /Pages 2 0 R /AcroForm 5 0 R >>",
            Some("<< /Fields 6 0 R >>"),
        ),
    ] {
        let mut objects = vec![
            (1, catalog),
            (2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
            (3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>"),
            (4, "<< /FT /Tx /T (Signature1) >>"),
        ];
        if let Some(acroform_obj) = acroform_obj {
            objects.push((5, acroform_obj));
            objects.push((6, "[4 0 R]"));
        }
        let doc =
            PdfDocument::from_bytes(sign(&pdf(&objects), &SigningOptions::default())).unwrap();
        let fields = refs(&doc, &acroform(&doc), b"Fields");
        assert_eq!(fields.len(), 2, "{catalog}");
        assert_eq!(fields[0], obj(4), "{catalog}");
        let sigs = detect_signatures(&doc).unwrap();
        assert_eq!(sigs.len(), 1, "{catalog}");
        assert_eq!(sigs[0].field_name, "Signature2", "{catalog}");
    }
}

#[test]
fn existing_annotations_are_kept() {
    for (page, annots_obj) in [
        (
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Annots [4 0 R] >>",
            None,
        ),
        (
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Annots 5 0 R >>",
            Some("[4 0 R]"),
        ),
    ] {
        let mut objects = vec![
            (1, "<< /Type /Catalog /Pages 2 0 R >>"),
            (2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
            (3, page),
            (4, "<< /Type /Annot /Subtype /Text /Rect [0 0 10 10] >>"),
        ];
        if let Some(annots_obj) = annots_obj {
            objects.push((5, annots_obj));
        }
        let doc =
            PdfDocument::from_bytes(sign(&pdf(&objects), &SigningOptions::default())).unwrap();
        let annots = refs(&doc, &dict(&doc, 3), b"Annots");
        assert_eq!(annots.len(), 2, "{page}");
        assert_eq!(annots[0], obj(4), "{page}");
        assert_eq!(
            annots[1],
            refs(&doc, &acroform(&doc), b"Fields")[0],
            "{page}"
        );
    }
}

const TWO_PAGES: &[(u32, &str)] = &[
    (1, "<< /Type /Catalog /Pages 2 0 R >>"),
    (2, "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>"),
    (3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>"),
    (4, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>"),
];

#[test]
fn the_widget_goes_on_the_chosen_page_only() {
    let options = SigningOptions {
        page: 1,
        ..Default::default()
    };
    let doc = PdfDocument::from_bytes(sign(&pdf(TWO_PAGES), &options)).unwrap();
    assert!(refs(&doc, &dict(&doc, 3), b"Annots").is_empty());
    let annots = refs(&doc, &dict(&doc, 4), b"Annots");
    assert_eq!(annots.len(), 1);
    let widget = doc.resolve(&annots[0]).unwrap();
    assert_eq!(
        widget.as_dict().unwrap().get(b"P"),
        Some(&PdfObject::Reference(obj(4)))
    );
}

#[test]
fn a_page_past_the_last_is_refused() {
    let options = SigningOptions {
        page: 2,
        ..Default::default()
    };
    let err = sign_pdf(&pdf(TWO_PAGES), KEY, &[CERT], &options).unwrap_err();
    assert!(
        matches!(err, JustPdfError::PageOutOfRange { index: 2, count: 2 }),
        "{err:?}"
    );
}

#[test]
fn signing_twice_keeps_the_first_signature_valid() {
    let once = sign(&pdf(ONE_PAGE), &SigningOptions::default());
    let twice = sign(&once, &SigningOptions::default());
    assert_eq!(&twice[..once.len()], &once[..]);
    let doc = PdfDocument::from_bytes(twice.clone()).unwrap();
    let sigs = detect_signatures(&doc).unwrap();
    let names: Vec<&str> = sigs.iter().map(|s| s.field_name.as_str()).collect();
    assert_eq!(names, ["Signature1", "Signature2"]);
    for sig in &sigs {
        let result = verify_signature(&twice, sig).unwrap();
        assert!(
            result.digest_valid,
            "{}: {:?}",
            sig.field_name, result.validity
        );
        assert!(
            result.signature_valid,
            "{}: {:?}",
            sig.field_name, result.validity
        );
    }
}

#[test]
fn new_objects_are_numbered_above_every_existing_number() {
    // Four objects, the last numbered 16, listed in five xref entries.
    let objects: &[(u32, &str)] = &[
        (1, "<< /Type /Catalog /Pages 2 0 R >>"),
        (2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        (
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 16 0 R >>",
        ),
        (16, "<< /Length 0 >>\nstream\n\nendstream"),
    ];
    let doc = PdfDocument::from_bytes(sign(&pdf(objects), &SigningOptions::default())).unwrap();
    let field = refs(&doc, &acroform(&doc), b"Fields")[0].clone();
    assert!(field.obj_num > 16, "{field:?}");
    assert!(
        matches!(doc.resolve(&obj(16)).unwrap(), PdfObject::Stream { .. }),
        "object 16 is still the stream"
    );
}

#[test]
fn references_to_nothing_or_the_wrong_kind_are_not_revised() {
    for (catalog, page) in [
        // /AcroForm names an object that does not exist.
        (
            "<< /Type /Catalog /Pages 2 0 R /AcroForm 9 0 R >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>",
        ),
        // /AcroForm names a dictionary at a generation it lacks.
        (
            "<< /Type /Catalog /Pages 2 0 R /AcroForm 2 1 R >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>",
        ),
        // /Annots names the catalog, then the page tree at a generation it lacks.
        (
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Annots 1 0 R >>",
        ),
        (
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Annots 2 1 R >>",
        ),
    ] {
        let objects = [
            (1, catalog),
            (2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
            (3, page),
        ];
        let doc =
            PdfDocument::from_bytes(sign(&pdf(&objects), &SigningOptions::default())).unwrap();
        assert_eq!(
            justpdf_core::page::collect_pages(&doc).unwrap().len(),
            1,
            "{page}"
        );
        assert_eq!(
            dict(&doc, 1).get_name(b"Type"),
            Some(b"Catalog".as_slice()),
            "{page}"
        );
        assert_eq!(
            dict(&doc, 2).get_name(b"Type"),
            Some(b"Pages".as_slice()),
            "{page}"
        );
        assert_eq!(
            detect_signatures(&doc).unwrap().len(),
            1,
            "{catalog} {page}"
        );
        assert_eq!(refs(&doc, &dict(&doc, 3), b"Annots").len(), 1, "{page}");
    }
}

#[test]
fn a_field_name_in_utf_16_is_taken_too() {
    // `/T` is "Signature1" as UTF-16BE with a byte order mark.
    let name: String = "Signature1"
        .chars()
        .map(|c| format!("00{:02X}", c as u8))
        .collect();
    let field = format!("<< /FT /Tx /T <FEFF{name}> >>");
    let objects = [
        (
            1,
            "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [4 0 R] >> >>",
        ),
        (2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        (3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>"),
        (4, field.as_str()),
    ];
    let doc = PdfDocument::from_bytes(sign(&pdf(&objects), &SigningOptions::default())).unwrap();
    assert_eq!(detect_signatures(&doc).unwrap()[0].field_name, "Signature2");
}
