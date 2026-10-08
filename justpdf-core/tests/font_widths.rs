//! Text extraction reads a simple font's indirect `/Widths` and `/FirstChar`.

use justpdf_core::PdfDocument;
use justpdf_core::page::get_page;
use justpdf_core::text::extract_page_text;

/// A one-page PDF showing `(ABC)` at 10 pt from x = 10 in Helvetica as `/F1`,
/// whose dictionary ends with `entries`; `extra` are objects numbered from 6.
fn pdf(entries: &str, extra: &[&str]) -> PdfDocument {
    let font = format!("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica {entries} >>");
    let content = "BT /F1 10 Tf 10 100 Td (ABC) Tj ET";
    let stream = format!(
        "<< /Length {} >>\nstream\n{content}\nendstream",
        content.len()
    );
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_string(),
        font,
        stream,
    ];
    objects.extend(extra.iter().map(|s| s.to_string()));

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

/// The x of each extracted character, rounded to 0.01 pt.
fn char_xs(entries: &str, extra: &[&str]) -> Vec<f64> {
    let doc = pdf(entries, extra);
    let page = get_page(&doc, 0).unwrap();
    extract_page_text(&doc, &page)
        .unwrap()
        .chars
        .iter()
        .map(|c| (c.x * 100.0).round() / 100.0)
        .collect()
}

#[test]
fn indirect_widths_place_the_glyphs() {
    // A, B, C are 100, 2000 and 300 units: at 10 pt, B starts 1 pt after A
    // and C 20 pt after B.
    let xs = char_xs(
        "/FirstChar 65 /LastChar 67 /Widths 6 0 R",
        &["[100 2000 300]"],
    );
    assert_eq!(xs, vec![10.0, 11.0, 31.0]);
}

#[test]
fn an_indirect_first_char_is_read() {
    let xs = char_xs(
        "/FirstChar 6 0 R /LastChar 67 /Widths [100 2000 300]",
        &["65"],
    );
    assert_eq!(xs, vec![10.0, 11.0, 31.0]);
}

#[test]
fn indirect_width_elements_are_read() {
    let xs = char_xs(
        "/FirstChar 65 /LastChar 67 /Widths [100 6 0 R 300]",
        &["2000"],
    );
    assert_eq!(xs, vec![10.0, 11.0, 31.0]);
}

#[test]
fn a_widths_reference_that_does_not_resolve_is_ignored() {
    // Object 9 does not exist: Helvetica's built-in widths apply, as with no /Widths.
    let unresolved = char_xs("/FirstChar 65 /LastChar 67 /Widths 9 0 R", &[]);
    assert_eq!(unresolved, char_xs("", &[]));
}
