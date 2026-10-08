//! Text extraction advances a Type0 font by its descendant CID font's `/W`
//! and `/DW`, read in core.

use justpdf_core::PdfDocument;
use justpdf_core::page::get_page;
use justpdf_core::text::extract_page_text;

/// A one-page PDF showing the hex string `shown` at 10 pt from x = 10 in a
/// Type0 `/Identity-H` font (object 4) whose `/DescendantFonts` value is
/// `descendants`; `extra` are objects numbered from 6.
fn pdf(descendants: &str, shown: &str, extra: &[&str]) -> PdfDocument {
    let font = format!(
        "<< /Type /Font /Subtype /Type0 /BaseFont /CIDTest /Encoding /Identity-H \
         /DescendantFonts {descendants} >>"
    );
    let content = format!("BT /F1 10 Tf 10 100 Td <{shown}> Tj ET");
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

/// A CIDFontType2 dictionary with no font program, ending with `entries`.
fn cid_font(entries: &str) -> String {
    format!(
        "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /CIDTest \
         /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> {entries} >>"
    )
}

/// The x of each extracted character, rounded to 0.01 pt.
fn char_xs(doc: &PdfDocument) -> Vec<f64> {
    let page = get_page(doc, 0).unwrap();
    extract_page_text(doc, &page)
        .unwrap()
        .chars
        .iter()
        .map(|c| (c.x * 100.0).round() / 100.0)
        .collect()
}

/// `char_xs` with the descendant font `cid` as object 6.
fn xs(cid: &str, shown: &str, extra: &[&str]) -> Vec<f64> {
    let mut objects = vec![cid];
    objects.extend_from_slice(extra);
    char_xs(&pdf("[6 0 R]", shown, &objects))
}

#[test]
fn both_w_forms_place_the_glyphs() {
    // CID 1 is 2000 units (list form), CID 2 is 500 (range form).
    let cid = cid_font("/DW 1000 /W [1 [2000] 2 2 500]");
    assert_eq!(xs(&cid, "000100020001", &[]), vec![10.0, 30.0, 35.0]);
}

#[test]
fn a_cid_outside_w_advances_by_dw_or_1000() {
    assert_eq!(xs(&cid_font("/DW 300"), "00050005", &[]), vec![10.0, 13.0]);
    assert_eq!(xs(&cid_font(""), "00050005", &[]), vec![10.0, 20.0]);
}

#[test]
fn an_indirect_w_is_read() {
    let cid = cid_font("/W 7 0 R");
    assert_eq!(
        xs(&cid, "000100020001", &["[1 [2000] 2 2 500]"]),
        vec![10.0, 30.0, 35.0]
    );
}

#[test]
fn indirect_values_inside_w_are_read() {
    let cid = cid_font("/W [1 7 0 R 2 2 8 0 R]");
    assert_eq!(
        xs(&cid, "000100020001", &["[2000]", "500"]),
        vec![10.0, 30.0, 35.0]
    );
}

#[test]
fn an_indirect_width_inside_a_w_list_is_read() {
    let cid = cid_font("/W [1 [7 0 R] 2 2 500]");
    assert_eq!(xs(&cid, "000100020001", &["2000"]), vec![10.0, 30.0, 35.0]);
}

#[test]
fn a_non_number_in_a_w_list_keeps_the_widths_after_it_in_place() {
    // CID 2 takes the name's place (0 wide); CID 3 is still 500.
    let cid = cid_font("/W [1 [2000 /bad 500]]");
    assert_eq!(xs(&cid, "000100030001", &[]), vec![10.0, 30.0, 35.0]);
}

#[test]
fn a_direct_descendant_font_is_read() {
    let cid = cid_font("/W [1 [2000] 2 2 500]");
    let doc = pdf(&format!("[{cid}]"), "000100020001", &[]);
    assert_eq!(char_xs(&doc), vec![10.0, 30.0, 35.0]);
}
