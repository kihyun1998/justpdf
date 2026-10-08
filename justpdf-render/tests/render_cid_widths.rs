//! The raster and SVG renderers advance a Type0 font by its descendant CID
//! font's `/W` and `/DW`.

mod common;

use common::{Plain, RED, WHITE, page, rgb};
use justpdf_core::PdfDocument;
use justpdf_render::render_page_to_svg;

/// A page showing the hex string `shown` in red at 10 pt from (10, 40) in a
/// Type0 `/Identity-H` font over a CIDFontType2 with no font program, so each
/// glyph is a box as wide as its width; the CID font's dictionary ends with
/// `entries`.
fn show(entries: &str, shown: &str) -> PdfDocument {
    let cid = format!(
        "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /CIDTest \
         /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> {entries} >>"
    );
    page(
        "<< /Font << /F1 5 0 R >> >>",
        &format!("BT /F1 10 Tf 1 0 0 rg 10 40 Td <{shown}> Tj ET"),
        vec![
            Plain(
                "<< /Type /Font /Subtype /Type0 /BaseFont /CIDTest /Encoding /Identity-H \
                 /DescendantFonts [6 0 R] >>",
            ),
            Plain(&cid),
        ],
    )
}

#[test]
fn a_raster_box_is_as_wide_as_w() {
    // CID 1 is 2000 units: its box spans x 10–30 (y 58 is inside it).
    let doc = show("/DW 1000 /W [1 [2000]]", "0001");
    assert_eq!(rgb(&doc, 25, 58), RED);
    assert_eq!(rgb(&doc, 35, 58), WHITE);
}

#[test]
fn raster_glyphs_advance_by_both_w_forms() {
    // CID 1 spans x 10–30, CID 2 (500 units) x 30–35, then CID 1 again from 35.
    let doc = show("/W [1 [2000] 2 2 500]", "000100020001");
    assert_eq!(rgb(&doc, 32, 58), RED);
    assert_eq!(rgb(&doc, 52, 58), RED);
    assert_eq!(rgb(&doc, 57, 58), WHITE);
}

#[test]
fn a_raster_cid_outside_w_advances_by_1000_without_dw() {
    let doc = show("", "00050005");
    assert_eq!(rgb(&doc, 25, 58), RED);
    assert_eq!(rgb(&doc, 32, 58), WHITE);
}

#[test]
fn an_svg_placeholder_is_as_wide_as_w() {
    // CID 0x101 has no text in SVG, so it is drawn as a 20-unit-wide box.
    let svg = render_page_to_svg(&show("/W [257 [2000]]", "0101"), 0).unwrap();
    assert!(svg.contains("M0 -2 L20 -2"), "{svg}");
}
