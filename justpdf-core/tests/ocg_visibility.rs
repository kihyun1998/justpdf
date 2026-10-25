//! `ocg::is_oc_visible`: an `/OC` value (an OCG or an OCMD, inline or by
//! reference) under the default configuration, including `/VE` visibility
//! expressions (#44).

use std::io::Write;

use justpdf_core::PdfDocument;
use justpdf_core::object::{IndirectRef, PdfObject};
use justpdf_core::ocg::{self, OCConfig};

/// A document whose OCG 4 is ON and OCG 5 is OFF in `/D`, holding `extra` as
/// objects 6, 7, …
fn doc(extra: &[&str]) -> (PdfDocument, OCConfig) {
    let mut objs: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R /OCProperties << /OCGs [4 0 R 5 0 R] \
         /D << /ON [4 0 R] /OFF [5 0 R] >> >> >>"
            .into(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] >>".into(),
        "<< /Type /OCG /Name (On) >>".into(),
        "<< /Type /OCG /Name (Off) >>".into(),
    ];
    objs.extend(extra.iter().map(|s| s.to_string()));
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        write!(out, "{} 0 obj\n{o}\nendobj\n", i + 1).unwrap();
    }
    let xref = out.len();
    write!(out, "xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).unwrap();
    for off in offsets {
        writeln!(out, "{off:010} 00000 n ").unwrap();
    }
    write!(
        out,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objs.len() + 1
    )
    .unwrap();
    let doc = PdfDocument::from_bytes(out).unwrap();
    let config = ocg::read_oc_properties(&doc)
        .unwrap()
        .unwrap()
        .default_config
        .unwrap();
    (doc, config)
}

fn r(n: u32) -> PdfObject {
    PdfObject::Reference(IndirectRef {
        obj_num: n,
        gen_num: 0,
    })
}

/// Whether object 6 of a document holding `ocmd` is visible.
fn ocmd_visible(ocmd: &str) -> bool {
    let (doc, config) = doc(&[ocmd]);
    ocg::is_oc_visible(&r(6), &doc, &config)
}

#[test]
fn an_ocg_by_reference() {
    let (doc, config) = doc(&[]);
    assert!(ocg::is_oc_visible(&r(4), &doc, &config));
    assert!(!ocg::is_oc_visible(&r(5), &doc, &config));
}

#[test]
fn an_ocmd_policy() {
    assert!(!ocmd_visible(
        "<< /Type /OCMD /OCGs [4 0 R 5 0 R] /P /AllOn >>"
    ));
    assert!(ocmd_visible(
        "<< /Type /OCMD /OCGs [4 0 R 5 0 R] /P /AnyOn >>"
    ));
    assert!(ocmd_visible("<< /Type /OCMD /OCGs [5 0 R] /P /AllOff >>"));
    // /P defaults to AnyOn.
    assert!(!ocmd_visible("<< /Type /OCMD /OCGs 5 0 R >>"));
}

#[test]
fn an_ocmd_without_ocgs_has_no_effect() {
    assert!(ocmd_visible("<< /Type /OCMD /P /AllOn >>"));
}

#[test]
fn visibility_expressions() {
    assert!(ocmd_visible("<< /Type /OCMD /VE [/And 4 0 R] >>"));
    assert!(!ocmd_visible("<< /Type /OCMD /VE [/And 4 0 R 5 0 R] >>"));
    assert!(ocmd_visible("<< /Type /OCMD /VE [/Or 5 0 R 4 0 R] >>"));
    assert!(ocmd_visible("<< /Type /OCMD /VE [/Not 5 0 R] >>"));
    assert!(!ocmd_visible(
        "<< /Type /OCMD /VE [/Not [/Or 4 0 R 5 0 R]] >>"
    ));
    assert!(ocmd_visible(
        "<< /Type /OCMD /VE [/And [/Not 5 0 R] [/Or 5 0 R 4 0 R]] >>"
    ));
}

#[test]
fn a_visibility_expression_takes_precedence_over_ocgs_and_p() {
    // /OCGs + /P alone would hide it (5 is OFF); /VE shows it.
    assert!(ocmd_visible(
        "<< /Type /OCMD /OCGs [5 0 R] /P /AllOn /VE [/Or 4 0 R] >>"
    ));
}

#[test]
fn anything_else_is_visible() {
    let (doc, config) = doc(&["<< /Foo 1 >>"]);
    assert!(ocg::is_oc_visible(&r(6), &doc, &config));
    assert!(ocg::is_oc_visible(&r(99), &doc, &config));
    assert!(ocg::is_oc_visible(&PdfObject::Integer(3), &doc, &config));
}
