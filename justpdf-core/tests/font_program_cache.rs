//! `PdfDocument::font_program` decodes a font-file stream once per document
//! and shares it; failures are kept too, and `authenticate` clears them all.

use std::sync::Arc;

use justpdf_core::PdfDocument;
use justpdf_core::font::font_hash;
use justpdf_core::object::IndirectRef;

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// A PDF whose object `n` (1-based) is `objects[n - 1]`.
fn pdf(objects: &[&str]) -> PdfDocument {
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
    }
    let xref_at = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    PdfDocument::from_bytes(out).unwrap()
}

fn r(obj_num: u32) -> IndirectRef {
    IndirectRef {
        obj_num,
        gen_num: 0,
    }
}

/// Object 2 is a font-file stream `ABC` (hex-encoded), object 3 one whose
/// Flate data is corrupt, object 4 a dictionary.
fn doc() -> PdfDocument {
    pdf(&[
        "<< /Type /Catalog >>",
        "<< /Filter /ASCIIHexDecode /Length 7 >>\nstream\n414243>\nendstream",
        "<< /Filter /FlateDecode /Length 8 >>\nstream\nnotflate\nendstream",
        "<< /Not /AStream >>",
    ])
}

#[test]
fn a_font_program_is_decoded_once_and_shared() {
    let doc = doc();
    let first = doc.font_program(&r(2)).unwrap();
    let second = doc.font_program(&r(2)).unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(&*first.data, b"ABC");
    assert_eq!(first.hash, font_hash(b"ABC"));
    assert_eq!(doc.font_program_decodes(), 1);
}

#[test]
fn a_failure_is_kept_and_not_retried() {
    let doc = doc();
    assert!(doc.font_program(&r(3)).is_err());
    assert!(doc.font_program(&r(3)).is_err());
    assert!(doc.font_program(&r(4)).is_err());
    assert!(doc.font_program(&r(4)).is_err());
    assert_eq!(doc.font_program_decodes(), 2);
}

#[test]
fn authenticate_clears_the_cache_and_refills_it_decrypted() {
    let open = || {
        PdfDocument::from_bytes(std::fs::read(fixture("aes256_r5_user_owner.pdf")).unwrap())
            .unwrap()
    };
    // The page's content stream, found through an authenticated copy.
    let mut unlocked_copy = open();
    unlocked_copy.authenticate(b"userpw").unwrap();
    let page = justpdf_core::page::get_page(&unlocked_copy, 0).unwrap();
    let Some(justpdf_core::object::PdfObject::Reference(contents)) = page.contents_ref.clone()
    else {
        panic!("contents is not a reference: {:?}", page.contents_ref);
    };

    let mut doc = open();
    // Before authenticating, the stream does not decode to the plaintext.
    let locked = doc.font_program(&contents).ok();
    assert!(
        locked
            .as_ref()
            .is_none_or(|p| !p.data.windows(6).any(|w| w == b"secret")),
        "decrypted before authenticate"
    );

    doc.authenticate(b"userpw").unwrap();
    let unlocked = doc.font_program(&contents).unwrap();
    assert!(unlocked.data.windows(6).any(|w| w == b"secret"));
    assert_eq!(doc.font_program_decodes(), 2);
}
