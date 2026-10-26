//! A pattern's `/Matrix` maps pattern space to the default space of the
//! content stream it is painted in, the CTM at that stream's start: a `cm`
//! before the paint does not move it, a form's matrix and the CTM at its
//! `Do` do (#397).

mod common;

use common::{RED, Stream, WHITE, page, rgb};

/// Red at x = 0 to blue at x = 100 (in the space it is painted in).
const RAMP: &str = "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [0 0 100 0] \
     /Function << /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >> >>";

/// Black at x = 0 to white at x = 100.
const GRAY_RAMP: &str = "<< /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 100 0] \
     /Function << /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 1 >> >>";

/// A page with shading `/X` and shading pattern `/PX` over the ramp, `/G`
/// and `/PG` over the gray ramp, tiling patterns `/T` (object 5: a 20 × 20
/// cell with a red 10 × 5 strip at its origin) and `/T2` (object 8: a
/// 100 × 100 cell filled with `/PX`), forms `/Fm` (object 6) and `/Fg`
/// (object 7, a transparency group), both with `form_matrix` and
/// `form_content`, which can use them all, and `/Sm`, a luminosity soft mask
/// drawn by `/Fg`.
fn doc(content: &str, form_matrix: &str, form_content: &str) -> justpdf_core::PdfDocument {
    let names = format!(
        "/Shading << /X {RAMP} /G {GRAY_RAMP} >> \
         /Pattern << /PX << /PatternType 2 /Shading {RAMP} >> \
         /PG << /PatternType 2 /Shading {GRAY_RAMP} >> /T 5 0 R /T2 8 0 R >>"
    );
    let resources = format!(
        "<< {names} /XObject << /Fm 6 0 R /Fg 7 0 R >> \
         /ExtGState << /Sm << /SMask << /S /Luminosity /G 7 0 R >> >> >> >>"
    );
    let tile_of_ramp = format!(
        "/PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 100 100] \
         /XStep 100 /YStep 100 /Resources << {names} >>"
    );
    let tiling = "/PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 20 20] \
         /XStep 20 /YStep 20 /Resources << >>";
    let form = format!(
        "/Type /XObject /Subtype /Form /BBox [-100 -100 200 200] \
         /Matrix {form_matrix} /Resources << {names} >>"
    );
    let group = format!("{form} /Group << /S /Transparency >>");
    page(
        &resources,
        content,
        vec![
            Stream(tiling, "1 0 0 rg 0 0 10 5 re f"),
            Stream(&form, form_content),
            Stream(&group, form_content),
            Stream(&tile_of_ramp, "/Pattern cs /PX scn 0 0 100 100 re f"),
        ],
    )
}

/// Each listed pixel of `a` is the same pixel of `b`.
fn same_pixels(
    a: &justpdf_core::PdfDocument,
    b: &justpdf_core::PdfDocument,
    pixels: &[(u32, u32)],
) {
    for &(x, y) in pixels {
        assert_eq!(rgb(a, x, y), rgb(b, x, y), "({x}, {y})");
    }
}

/// RGB at user point (x, y) of the page at 72 dpi.
fn at(doc: &justpdf_core::PdfDocument, x: u32, y: u32) -> [u8; 3] {
    rgb(doc, x, 100 - y)
}

const IDENTITY: &str = "[1 0 0 1 0 0]";
const SHIFTED: &str = "[1 0 0 1 10 0]";
const ACROSS: [(u32, u32); 3] = [(15, 50), (50, 50), (90, 50)];

// ---- a cm in the same stream ----------------------------------------------------

#[test]
fn a_cm_before_a_shading_pattern_fill_does_not_move_the_pattern() {
    let fill = doc(
        "0.5 0 0 0.5 0 0 cm /Pattern cs /PX scn 0 0 200 200 re f",
        IDENTITY,
        "",
    );
    same_pixels(
        &fill,
        &doc("/X sh", IDENTITY, ""),
        &[(20, 50), (60, 50), (90, 50)],
    );
}

#[test]
fn a_cm_before_a_tiling_pattern_fill_does_not_move_the_pattern() {
    let fill = doc(
        "0.5 0 0 0.5 3 0 cm /Pattern cs /T scn 0 0 200 200 re f",
        IDENTITY,
        "",
    );
    // The strip stays at x 0..10 and y 0..5 of each 20-unit cell of the page.
    assert_eq!(at(&fill, 5, 2), RED);
    assert_eq!(at(&fill, 15, 2), WHITE);
    assert_eq!(at(&fill, 5, 8), WHITE);
    assert_eq!(at(&fill, 25, 22), RED);
    assert_eq!(at(&fill, 65, 82), RED);
}

#[test]
fn a_cm_before_a_tiling_pattern_stroke_does_not_move_the_pattern() {
    let stroke = doc(
        "2 0 0 2 0 0 cm 15 w /Pattern CS /T SCN 0 7.5 m 50 7.5 l S",
        IDENTITY,
        "",
    );
    assert_eq!(at(&stroke, 5, 2), RED);
    assert_eq!(at(&stroke, 15, 2), WHITE);
    assert_eq!(at(&stroke, 25, 22), RED);
}

// ---- forms ----------------------------------------------------------------------

/// The ramp drawn by `sh` in the page space moved 10 units right.
fn shifted_ramp() -> justpdf_core::PdfDocument {
    doc("q 1 0 0 1 10 0 cm /X sh Q", IDENTITY, "")
}

#[test]
fn a_shading_pattern_in_a_form_follows_the_form_matrix() {
    let fill = doc("/Fm Do", SHIFTED, "/Pattern cs /PX scn -10 0 100 100 re f");
    same_pixels(&fill, &shifted_ramp(), &ACROSS);
}

#[test]
fn a_shading_pattern_in_a_group_form_follows_the_form_matrix() {
    let fill = doc("/Fg Do", SHIFTED, "/Pattern cs /PX scn -10 0 100 100 re f");
    same_pixels(&fill, &shifted_ramp(), &ACROSS);
}

#[test]
fn the_ctm_at_do_moves_a_pattern_in_the_form() {
    let fill = doc(
        "1 0 0 1 10 0 cm /Fm Do",
        IDENTITY,
        "/Pattern cs /PX scn -10 0 100 100 re f",
    );
    same_pixels(&fill, &shifted_ramp(), &ACROSS);
}

#[test]
fn a_cm_inside_a_form_does_not_move_its_pattern() {
    let fill = doc(
        "/Fm Do",
        SHIFTED,
        "0.5 0 0 0.5 0 0 cm /Pattern cs /PX scn -20 0 200 200 re f",
    );
    same_pixels(&fill, &shifted_ramp(), &ACROSS);
}

#[test]
fn a_tiling_pattern_in_a_form_follows_the_form_matrix() {
    let fill = doc(
        "/Fm Do",
        SHIFTED,
        "1 0 0 1 3 0 cm /Pattern cs /T scn -20 0 120 100 re f",
    );
    // The strip covers x 10..20, 30..40, ... of the page, not 13..23.
    assert_eq!(at(&fill, 11, 2), RED);
    assert_eq!(at(&fill, 21, 2), WHITE);
    assert_eq!(at(&fill, 5, 2), WHITE);
    assert_eq!(at(&fill, 31, 22), RED);
}

#[test]
fn the_page_keeps_its_pattern_space_after_a_form() {
    let fill = doc("/Fm Do /Pattern cs /PX scn 0 0 100 100 re f", SHIFTED, "");
    same_pixels(
        &fill,
        &doc("/X sh", IDENTITY, ""),
        &[(20, 50), (60, 50), (90, 50)],
    );
}

#[test]
fn a_pattern_in_a_soft_mask_form_follows_the_form_matrix() {
    let masked = doc(
        "/Sm gs 1 0 0 rg 0 0 100 100 re f",
        SHIFTED,
        "/Pattern cs /PG scn -10 0 100 100 re f",
    );
    let reference = doc(
        "/Sm gs 1 0 0 rg 0 0 100 100 re f",
        IDENTITY,
        "q 1 0 0 1 10 0 cm /G sh Q",
    );
    same_pixels(&masked, &reference, &ACROSS);
}

#[test]
fn a_pattern_in_a_tile_follows_the_tile_not_the_form_around_it() {
    // `/T2`'s cell paints `/PX` in pattern space, whatever stream the tiling
    // is painted from.
    let fill = doc("/Fm Do", SHIFTED, "/Pattern cs /T2 scn -10 0 100 100 re f");
    same_pixels(&fill, &shifted_ramp(), &ACROSS);
}
