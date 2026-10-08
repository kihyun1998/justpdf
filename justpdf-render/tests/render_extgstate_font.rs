//! An ExtGState's `/Font [font size]`, applied with `gs`, sets the text font
//! and size in the raster and SVG renderers, as `Tf` would.

mod common;

use common::{Plain, page};
use justpdf_core::PdfDocument;
use justpdf_render::{RenderOptions, render_page_to_pixmap, render_page_to_svg};

/// A simple font named `base` whose `A`, `B` and `C` are `width` units wide.
fn font(base: &str, width: u32) -> String {
    format!(
        "<< /Type /Font /Subtype /Type1 /BaseFont /{base} /FirstChar 65 /LastChar 67 \
         /Widths [{width} {width} {width}] >>"
    )
}

/// A page running `content`. `/F1` is Helvetica (500 units, object 5) and
/// `/F2` Courier (600, object 6); object 7 is Times-Roman (700), named in no
/// `/Font`. `/GS2` sets Courier at 12 and `/GS3` Times-Roman at 20;
/// `/BadN` are malformed `/Font` entries.
fn doc(content: &str) -> PdfDocument {
    let resources = "<< /Font << /F1 5 0 R /F2 6 0 R >> /ExtGState << \
         /GS2 << /Font [6 0 R 12] >> /GS3 << /Font [7 0 R 20] >> \
         /Bad1 << /Font [6 0 R] >> /Bad2 << /Font [/F2 12] >> \
         /Bad3 << /Font [99 0 R 12] >> /Bad4 << /Font [6 0 R /x] >> \
         /Bad5 << /Font 6 0 R >> >> >>";
    let helvetica = font("Helvetica", 500);
    let courier = font("Courier", 600);
    let times = font("Times-Roman", 700);
    page(
        resources,
        content,
        vec![Plain(&helvetica), Plain(&courier), Plain(&times)],
    )
}

fn svg(content: &str) -> String {
    render_page_to_svg(&doc(content), 0).unwrap()
}

#[test]
fn svg_text_after_gs_alone_uses_its_font_and_size() {
    // Courier at 12 advances 7.2 per glyph from x = 10.
    let svg = svg("q /GS2 gs BT 10 50 Td (ABC) Tj ET Q");
    assert!(svg.contains("x=\"17.2\""), "{svg}");
    assert!(svg.contains("x=\"24.4\""), "{svg}");
    assert!(svg.contains("font-size=\"12\""), "{svg}");
}

#[test]
fn svg_gs_font_replaces_the_tf_font() {
    // A in Helvetica 10 ends at 15; then Courier 12: 15, 22.2, 29.4.
    let svg = svg("BT /F1 10 Tf 10 50 Td (A) Tj /GS2 gs (ABC) Tj ET");
    assert!(svg.contains("x=\"22.2\""), "{svg}");
    assert!(svg.contains("x=\"29.4\""), "{svg}");
}

#[test]
fn svg_gs_font_need_not_be_in_the_font_resources() {
    // Times-Roman at 20 advances 14 per glyph.
    let svg = svg("BT /F1 10 Tf ET q /GS3 gs BT 10 50 Td (AB) Tj ET Q");
    assert!(svg.contains("x=\"24\""), "{svg}");
    assert!(svg.contains("font-size=\"20\""), "{svg}");
}

#[test]
fn svg_q_restores_the_font_from_before_gs() {
    // After Q the font is Helvetica 10 again: 10, 15.
    let svg = svg("BT /F1 10 Tf ET q /GS2 gs Q BT 10 50 Td (AB) Tj ET");
    assert!(svg.contains("x=\"15\""), "{svg}");
    assert!(!svg.contains("x=\"17.2\""), "{svg}");
}

#[test]
fn svg_a_malformed_gs_font_changes_nothing() {
    for bad in ["Bad1", "Bad2", "Bad3", "Bad4", "Bad5"] {
        // Helvetica 10 throughout: A at 10, then A, B at 15 and 20.
        let svg = svg(&format!(
            "BT /F1 10 Tf 10 50 Td (A) Tj /{bad} gs (AB) Tj ET"
        ));
        assert!(svg.contains("x=\"20\""), "{bad}: {svg}");
        assert!(!svg.contains("x=\"22.2\""), "{bad}: {svg}");
    }
}

#[test]
fn raster_text_after_gs_alone_is_drawn() {
    // Courier 12 from (10, 50): its glyphs ink rows above device row 50.
    let px = render_page_to_pixmap(
        &doc("q /GS2 gs BT 10 50 Td (ABC) Tj ET Q"),
        0,
        &RenderOptions::default(),
    )
    .unwrap();
    let inked = (40..51)
        .flat_map(|y| (10..32).map(move |x| (x, y)))
        .filter(|&(x, y)| px.data[((y * px.width + x) * 4) as usize] < 128)
        .count();
    assert!(inked > 0);
}
