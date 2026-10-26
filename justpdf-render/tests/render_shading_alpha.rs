//! `sh` and pattern fills and strokes paint with the graphics state's alpha
//! constant and blend mode (#391).

mod common;

use common::{RED, Stream, WHITE, page, rgb};

const BLUE: [u8; 3] = [0, 0, 255];
const HALF_RED: [u8; 3] = [255, 128, 128];

fn hex(data: &[u8]) -> String {
    let mut s: String = data.iter().map(|b| format!("{b:02X}")).collect();
    s.push('>');
    s
}

/// A red axial shading across the page.
const AXIAL: &str = "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [0 0 100 0] \
     /Function << /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [1 0 0] /N 1 >> >>";

const MESH: &str = "/ShadingType 4 /ColorSpace /DeviceRGB /BitsPerCoordinate 8 \
     /BitsPerComponent 8 /BitsPerFlag 8 /Decode [0 100 0 100 0 1 0 1 0 1] \
     /Filter /ASCIIHexDecode";

/// A red free-form mesh covering the page: two triangles, then `extra`
/// vertices.
fn mesh(extra: &[u8]) -> String {
    let mut data = vec![
        0, 0, 0, 255, 0, 0, //
        0, 255, 0, 255, 0, 0, //
        0, 0, 255, 255, 0, 0, //
        0, 255, 0, 255, 0, 0, //
        0, 255, 255, 255, 0, 0, //
        0, 0, 255, 255, 0, 0,
    ];
    data.extend_from_slice(extra);
    hex(&data)
}

/// A page with `/Ax` (the axial shading), `/Me` (object 5, the mesh with
/// `mesh_data`), patterns `/P0` (the axial shading) and `/P1` (a red tiling
/// pattern, object 7) and graphics states
/// `/Fa` (`ca 0.5`), `/Sa` (`CA 0.5`), `/Mu` (Multiply) and `/Sm` (a
/// luminosity soft mask of half coverage, group object 6).
fn shaded(content: &str, mesh_data: &str) -> justpdf_core::PdfDocument {
    let resources = format!(
        "<< /Shading << /Ax {AXIAL} /Me 5 0 R >> \
         /Pattern << /P0 << /PatternType 2 /Shading {AXIAL} >> /P1 7 0 R >> \
         /ExtGState << /Fa << /ca 0.5 >> /Sa << /CA 0.5 >> /Mu << /BM /Multiply >> \
         /Sm << /SMask << /S /Luminosity /G 6 0 R >> >> >> >>"
    );
    let group = "/Type /XObject /Subtype /Form /BBox [0 0 100 100] \
         /Group << /S /Transparency /CS /DeviceGray >>";
    let tiling = "/PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 10 10] \
         /XStep 10 /YStep 10 /Resources << >>";
    page(
        &resources,
        content,
        vec![
            Stream(MESH, mesh_data),
            Stream(group, "0.5 g 0 0 100 100 re f"),
            Stream(tiling, "1 0 0 rg 0 0 10 10 re f"),
        ],
    )
}

/// Within 3 per channel: the mesh rasterizer truncates (255 → 254).
fn near(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(&x, y)| x.abs_diff(y) <= 3)
}

const BLUE_PAGE: &str = "0 0 1 rg 0 0 100 100 re f";

// ---- sh -------------------------------------------------------------------------

#[test]
fn an_axial_sh_paints_with_the_fill_alpha() {
    let doc = shaded("/Fa gs /Ax sh", &mesh(&[]));
    assert_eq!(rgb(&doc, 30, 50), HALF_RED);
}

#[test]
fn an_axial_sh_paints_with_the_blend_mode() {
    let doc = shaded(&format!("{BLUE_PAGE} /Mu gs /Ax sh"), &mesh(&[]));
    assert_eq!(rgb(&doc, 30, 50), [0, 0, 0]);
}

#[test]
fn a_mesh_sh_paints_with_the_fill_alpha() {
    let doc = shaded("/Fa gs /Me sh", &mesh(&[]));
    let px = rgb(&doc, 30, 50);
    assert!(near(px, HALF_RED), "{px:?}");
}

#[test]
fn a_mesh_sh_paints_with_the_blend_mode() {
    let doc = shaded(&format!("{BLUE_PAGE} /Mu gs /Me sh"), &mesh(&[]));
    let px = rgb(&doc, 30, 50);
    assert!(near(px, [0, 0, 0]), "{px:?}");
}

#[test]
fn overlapping_mesh_triangles_take_the_fill_alpha_once() {
    // The first triangle again: the shading is one object, so its alpha is
    // applied once where its triangles overlap.
    let again = [
        0, 0, 0, 255, 0, 0, //
        0, 255, 0, 255, 0, 0, //
        0, 0, 255, 255, 0, 0,
    ];
    let doc = shaded("/Fa gs /Me sh", &mesh(&again));
    let px = rgb(&doc, 30, 50);
    assert!(near(px, HALF_RED), "{px:?}");
}

#[test]
fn a_translucent_sh_is_clipped() {
    let doc = shaded(
        &format!("{BLUE_PAGE} q 0 0 50 100 re W n /Fa gs /Ax sh Q"),
        &mesh(&[]),
    );
    // What a red fill with the same alpha gives over blue, to a unit: the
    // fill's alpha is truncated to 127/255.
    let fill = shaded(
        &format!("{BLUE_PAGE} /Fa gs 1 0 0 rg 0 0 100 100 re f"),
        &mesh(&[]),
    );
    let px = rgb(&doc, 25, 50);
    let want = rgb(&fill, 25, 50);
    assert!(
        px.iter().zip(want).all(|(&a, b)| a.abs_diff(b) <= 1),
        "{px:?} {want:?}"
    );
    assert_eq!(rgb(&doc, 75, 50), BLUE);
}

#[test]
fn a_translucent_sh_is_soft_masked_once() {
    // Alpha 0.5 under a half soft mask: red at a quarter over white.
    let doc = shaded("/Sm gs /Fa gs /Ax sh", &mesh(&[]));
    let px = rgb(&doc, 30, 50);
    assert!(near(px, [255, 191, 191]), "{px:?}");
}

#[test]
fn sh_ignores_the_stroke_alpha() {
    let doc = shaded("/Sa gs /Ax sh", &mesh(&[]));
    assert_eq!(rgb(&doc, 30, 50), RED);
}

// ---- shading patterns -----------------------------------------------------------

#[test]
fn a_shading_pattern_fill_paints_with_the_fill_alpha() {
    let doc = shaded("/Fa gs /Pattern cs /P0 scn 0 0 100 100 re f", &mesh(&[]));
    assert_eq!(rgb(&doc, 30, 50), HALF_RED);
}

#[test]
fn a_shading_pattern_stroke_paints_with_the_stroke_alpha() {
    let doc = shaded(
        "/Sa gs 20 w /Pattern CS /P0 SCN 0 50 m 100 50 l S",
        &mesh(&[]),
    );
    assert_eq!(rgb(&doc, 30, 50), HALF_RED);
    assert_eq!(rgb(&doc, 30, 10), WHITE);
}

#[test]
fn a_shading_pattern_fill_ignores_the_stroke_alpha() {
    let doc = shaded("/Sa gs /Pattern cs /P0 scn 0 0 100 100 re f", &mesh(&[]));
    assert_eq!(rgb(&doc, 30, 50), RED);
}

#[test]
fn a_tiling_pattern_fill_paints_with_the_fill_alpha() {
    let doc = shaded("/Fa gs /Pattern cs /P1 scn 0 0 100 100 re f", &mesh(&[]));
    assert_eq!(rgb(&doc, 35, 55), HALF_RED);
}
