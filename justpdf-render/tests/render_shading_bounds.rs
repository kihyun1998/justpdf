//! A clipped `sh` and a shading-pattern paint draw only the pixels they can
//! touch, and those pixels come out as on the whole page (#394).

mod common;

use common::{Stream, WHITE, page, rgb};

/// Red at x = 0 to blue at x = 100, and black at y = 0 to green at y = 100
/// (user space), so each pixel has its own colour.
const RAMP_X: &str = "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [0 0 100 0] \
     /Function << /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >> >>";
const RAMP_Y: &str = "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [0 0 0 100] \
     /Function << /FunctionType 2 /Domain [0 1] /C0 [0 0 0] /C1 [0 1 0] /N 1 >> >>";

const MESH: &str = "/ShadingType 4 /ColorSpace /DeviceRGB /BitsPerCoordinate 8 \
     /BitsPerComponent 8 /BitsPerFlag 8 /Decode [0 100 0 100 0 1 0 1 0 1] \
     /Filter /ASCIIHexDecode";

/// A free-form mesh over the page with a different colour at each corner.
const MESH_DATA: &str = "000000FF0000 00FF0000FF00 0000FF0000FF \
     00FF0000FF00 00FFFFFFFF00 0000FF0000FF>";

/// A page with shadings `/X`, `/Y` and `/M` (object 5) and shading patterns
/// `/PX` and `/PY` over the same ramps, and `/Fa` (`ca 0.5`).
fn shaded(content: &str) -> justpdf_core::PdfDocument {
    let resources = format!(
        "<< /Shading << /X {RAMP_X} /Y {RAMP_Y} /M 5 0 R >> \
         /Pattern << /PX << /PatternType 2 /Shading {RAMP_X} >> \
         /PY << /PatternType 2 /Shading {RAMP_Y} >> >> \
         /ExtGState << /Fa << /ca 0.5 >> >> >>"
    );
    page(&resources, content, vec![Stream(MESH, MESH_DATA)])
}

/// Each listed pixel of `clipped` is the same pixel of `whole`.
fn same_pixels(clipped: &str, whole: &str, pixels: &[(u32, u32)]) {
    near_pixels(clipped, whole, pixels, 0);
}

/// Each listed pixel of `clipped` is within `by` per channel of the same
/// pixel of `whole`.
fn near_pixels(clipped: &str, whole: &str, pixels: &[(u32, u32)], by: u8) {
    let (a, b) = (shaded(clipped), shaded(whole));
    for &(x, y) in pixels {
        let (p, q) = (rgb(&a, x, y), rgb(&b, x, y));
        assert!(
            p.iter().zip(q).all(|(&m, n)| m.abs_diff(n) <= by),
            "({x}, {y}): {p:?} {q:?}"
        );
    }
}

// ---- sh -------------------------------------------------------------------------

#[test]
fn a_clipped_sh_keeps_its_colours_across_x() {
    let clip = "30 0 30 100 re W n";
    same_pixels(
        &format!("{clip} /X sh"),
        "/X sh",
        &[(30, 50), (45, 50), (59, 50)],
    );
    let doc = shaded(&format!("{clip} /X sh"));
    assert_eq!(rgb(&doc, 20, 50), WHITE);
    assert_eq!(rgb(&doc, 70, 50), WHITE);
}

#[test]
fn a_clipped_sh_keeps_its_colours_across_y() {
    // User y 30..60 is device rows 40..70.
    let clip = "0 30 100 30 re W n";
    same_pixels(
        &format!("{clip} /Y sh"),
        "/Y sh",
        &[(50, 40), (50, 55), (50, 69)],
    );
    let doc = shaded(&format!("{clip} /Y sh"));
    assert_eq!(rgb(&doc, 50, 30), WHITE);
    assert_eq!(rgb(&doc, 50, 80), WHITE);
}

#[test]
fn a_clipped_mesh_keeps_its_colours() {
    // To a unit: the mesh rasterizer truncates its interpolated colours, and
    // the vertices move with the pixmap.
    let clip = "20 20 40 50 re W n";
    near_pixels(
        &format!("{clip} /M sh"),
        "/M sh",
        &[(22, 32), (40, 55), (58, 78)],
        1,
    );
    let doc = shaded(&format!("{clip} /M sh"));
    assert_eq!(rgb(&doc, 10, 50), WHITE);
    assert_eq!(rgb(&doc, 70, 50), WHITE);
}

#[test]
fn a_clipped_translucent_sh_keeps_its_colours() {
    same_pixels(
        "30 30 30 30 re W n /Fa gs /X sh",
        "/Fa gs /X sh",
        &[(31, 45), (45, 55), (58, 68)],
    );
}

#[test]
fn a_clipped_sh_under_a_moved_ctm_keeps_its_colours() {
    same_pixels(
        "1 0 0 1 7 -5 cm 30 30 30 30 re W n /X sh",
        "1 0 0 1 7 -5 cm /X sh",
        &[(38, 46), (50, 60), (65, 73)],
    );
}

#[test]
fn an_sh_under_an_empty_clip_paints_nothing() {
    let doc = shaded("50 50 0 0 re W n /X sh");
    assert_eq!(rgb(&doc, 50, 50), WHITE);
}

// ---- shading patterns -----------------------------------------------------------

#[test]
fn a_small_shading_pattern_fill_keeps_its_colours() {
    let fill = "/Pattern cs /PX scn 30 30 30 30 re f";
    same_pixels(fill, "/X sh", &[(31, 45), (45, 55), (58, 68)]);
    let doc = shaded(fill);
    assert_eq!(rgb(&doc, 20, 50), WHITE);
    assert_eq!(rgb(&doc, 70, 50), WHITE);
}

#[test]
fn a_shading_pattern_fill_keeps_its_colours_across_y() {
    same_pixels(
        "/Pattern cs /PY scn 30 30 30 30 re f",
        "/Y sh",
        &[(45, 41), (45, 55), (45, 68)],
    );
}

#[test]
fn a_clipped_shading_pattern_fill_keeps_its_colours() {
    same_pixels(
        "40 0 100 100 re W n /Pattern cs /PX scn 30 30 30 30 re f",
        "/X sh",
        &[(41, 45), (50, 55), (58, 68)],
    );
    let doc = shaded("40 0 100 100 re W n /Pattern cs /PX scn 30 30 30 30 re f");
    assert_eq!(rgb(&doc, 35, 50), WHITE);
}

#[test]
fn a_wide_shading_pattern_stroke_keeps_its_colours() {
    // A 20-wide horizontal stroke along user y = 50: rows 41..59.
    same_pixels(
        "20 w /Pattern CS /PX SCN 20 50 m 80 50 l S",
        "/X sh",
        &[(25, 42), (50, 50), (75, 58)],
    );
}

#[test]
fn a_shading_pattern_stroke_reaches_its_miter_tip() {
    // A sharp corner at (60, 50): the miter reaches about x = 75, well past
    // half the line width from the path.
    let stroke = "6 w 10 M /Pattern CS /PX SCN 10 40 m 60 50 l 10 60 l S";
    same_pixels(stroke, "/X sh", &[(70, 50)]);
    let doc = shaded(stroke);
    assert_ne!(rgb(&doc, 70, 50), WHITE);
}

/// RGB of device pixel (x, y) at `dpi`.
fn rgb_at(doc: &justpdf_core::PdfDocument, dpi: f64, x: u32, y: u32) -> [u8; 3] {
    let opts = justpdf_render::RenderOptions {
        dpi,
        ..Default::default()
    };
    let px = justpdf_render::render_page_to_pixmap(doc, 0, &opts).unwrap();
    let i = ((y * px.width + x) * 4) as usize;
    [px.data[i], px.data[i + 1], px.data[i + 2]]
}

#[test]
fn a_shading_pattern_fill_is_placed_once_at_any_resolution() {
    // At 144 dpi each user unit is two pixels; the pattern is not scaled again.
    let fill = shaded("/Pattern cs /PX scn 0 0 100 100 re f");
    let sh = shaded("/X sh");
    for (x, y) in [(40, 100), (100, 100), (160, 100)] {
        assert_eq!(
            rgb_at(&fill, 144.0, x, y),
            rgb_at(&sh, 144.0, x, y),
            "({x}, {y})"
        );
    }
}

#[test]
fn a_shading_pattern_fill_off_the_pixel_grid_keeps_its_edges() {
    // The rectangle's edges fall mid-pixel; those pixels are partly covered,
    // as by a clip of the same rectangle.
    same_pixels(
        "/Pattern cs /PX scn 30.5 30.5 29 29 re f",
        "30.5 30.5 29 29 re W n /X sh",
        &[(30, 45), (59, 45), (45, 40), (45, 69), (45, 55)],
    );
}
