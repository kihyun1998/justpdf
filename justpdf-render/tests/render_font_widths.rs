//! The raster and SVG renderers read a simple font's indirect `/Widths`.

mod common;

use common::{Plain, RED, WHITE, page, rgb};
use justpdf_core::PdfDocument;
use justpdf_render::render_page_to_svg;

/// A page showing `(AB)` in red at 50 pt from (10, 40) in a Type3 font with
/// no `/CharProcs`, so each glyph is a box as wide as its width; `widths` is
/// the font's `/Widths` value and `rest` are objects 6 on.
fn show_ab(widths: &str, rest: Vec<common::Obj>) -> PdfDocument {
    let font = format!(
        "<< /Type /Font /Subtype /Type3 /FontBBox [0 0 1000 1000] \
         /FontMatrix [0.001 0 0 0.001 0 0] /CharProcs << >> \
         /Encoding << /Differences [65 /A /B] >> /FirstChar 65 /LastChar 66 /Widths {widths} >>"
    );
    let mut objects = vec![Plain(&font)];
    objects.extend(rest);
    page(
        "<< /Font << /F1 5 0 R >> >>",
        "BT /F1 50 Tf 1 0 0 rg 10 40 Td (AB) Tj ET",
        objects,
    )
}

#[test]
fn raster_glyphs_advance_by_indirect_widths() {
    // A is 200 units: its box ends at x = 20, B's (100 units) at x = 25.
    let doc = show_ab("6 0 R", vec![Plain("[200 100]")]);
    assert_eq!(rgb(&doc, 15, 50), RED);
    assert_eq!(rgb(&doc, 40, 50), WHITE);
}

#[test]
fn svg_glyphs_advance_by_indirect_widths() {
    // B starts 200 units × 50 pt = 10 pt after A.
    let svg = render_page_to_svg(&show_ab("6 0 R", vec![Plain("[200 100]")]), 0).unwrap();
    assert!(svg.contains("x=\"20\""), "{svg}");
}
