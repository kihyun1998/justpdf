//! A tiling pattern's cell runs with the alpha constants and blend mode in
//! force at the start of the stream the pattern is painted in; the paint's
//! own `ca`, `CA` and blend mode apply once, when the tile is painted (#393).

mod common;

use common::{Stream, page, rgb};

const BLUE: [u8; 3] = [0, 0, 255];

/// A page with tiling pattern `/T` (object 5: a 20 × 20 cell with a red
/// square at 0..10 and a blue one over it at 5..15), graphics states `/Fa`
/// (`ca 0.5`), `/Sa` (`CA 0.5`) and `/Mu` (Multiply), and forms `/Fm`
/// (object 6) and `/Fg` (object 7, a transparency group) drawing
/// `form_content`.
fn doc(content: &str, form_content: &str) -> justpdf_core::PdfDocument {
    let names = "/Pattern << /T 5 0 R >> \
         /ExtGState << /Fa << /ca 0.5 >> /Sa << /CA 0.5 >> /Mu << /BM /Multiply >> >>";
    let resources = format!("<< {names} /XObject << /Fm 6 0 R /Fg 7 0 R >> >>");
    let tiling = "/PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 20 20] \
         /XStep 20 /YStep 20 /Resources << >>";
    let form =
        format!("/Type /XObject /Subtype /Form /BBox [0 0 100 100] /Resources << {names} >>");
    let group = format!("{form} /Group << /S /Transparency >>");
    page(
        &resources,
        content,
        vec![
            Stream(tiling, "1 0 0 rg 0 0 10 10 re f 0 0 1 rg 5 5 10 10 re f"),
            Stream(&form, form_content),
            Stream(&group, form_content),
        ],
    )
}

/// RGB at user point (x, y) of the page at 72 dpi.
fn at(doc: &justpdf_core::PdfDocument, x: u32, y: u32) -> [u8; 3] {
    rgb(doc, x, 100 - y)
}

/// Within a unit per channel: the paint's alpha is 0.5 where a fill's is
/// 127/255.
fn near(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(&x, y)| x.abs_diff(y) <= 1)
}

const FILL: &str = "/Pattern cs /T scn 0 0 100 100 re f";

#[test]
fn a_translucent_tile_shows_only_the_top_of_its_overlaps() {
    // The cell is opaque: blue covers red where they overlap, and the tile is
    // half transparent over white.
    let doc = doc(&format!("/Fa gs {FILL}"), "");
    let overlap = at(&doc, 7, 7);
    assert!(near(overlap, [128, 128, 255]), "{overlap:?}");
    let red = at(&doc, 2, 2);
    assert!(near(red, [255, 128, 128]), "{red:?}");
    let blue = at(&doc, 12, 12);
    assert!(near(blue, [128, 128, 255]), "{blue:?}");
}

#[test]
fn an_opaque_tile_is_unchanged() {
    let doc = doc(FILL, "");
    assert_eq!(at(&doc, 7, 7), BLUE);
    assert_eq!(at(&doc, 2, 2), [255, 0, 0]);
}

#[test]
fn a_tile_takes_the_blend_mode_once() {
    // Multiply over white leaves the tile as drawn: blue over red, not their
    // product.
    let doc = doc(&format!("/Mu gs {FILL}"), "");
    assert_eq!(at(&doc, 7, 7), BLUE);
    assert_eq!(at(&doc, 2, 2), [255, 0, 0]);
}

#[test]
fn a_translucent_tile_stroke_takes_the_stroke_alpha() {
    let doc = doc("/Sa gs 30 w /Pattern CS /T SCN 0 15 m 100 15 l S", "");
    let overlap = at(&doc, 7, 7);
    assert!(near(overlap, [128, 128, 255]), "{overlap:?}");
}

#[test]
fn a_tile_in_a_group_form_runs_from_the_group_start() {
    // A group starts at alpha 1, so the cell is opaque inside it.
    let doc = doc("/Fg Do", &format!("/Fa gs {FILL}"));
    let overlap = at(&doc, 7, 7);
    assert!(near(overlap, [128, 128, 255]), "{overlap:?}");
}

#[test]
fn a_tile_in_a_form_runs_with_the_alpha_at_its_do() {
    // A plain form inherits `ca 0.5` at `Do`, so its stream starts there: the
    // cell is drawn at 0.5 and painted at 0.5 again, red at a quarter.
    let doc = doc("/Fa gs /Fm Do", FILL);
    let red = at(&doc, 2, 2);
    assert!(near(red, [255, 191, 191]), "{red:?}");
}
