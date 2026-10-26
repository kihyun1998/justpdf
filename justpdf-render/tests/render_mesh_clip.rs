//! Mesh shadings (types 4–7) paint through the clip and the soft mask, like
//! every other draw (#383).

mod common;

use common::{Obj, RED, Stream, WHITE, page, rgb};

fn hex(data: &[u8]) -> String {
    let mut s: String = data.iter().map(|b| format!("{b:02X}")).collect();
    s.push('>');
    s
}

/// A red free-form mesh covering the page: two triangles, flag, x, y, r, g, b.
fn free_form() -> String {
    hex(&[
        0, 0, 0, 255, 0, 0, //
        0, 255, 0, 255, 0, 0, //
        0, 0, 255, 255, 0, 0, //
        0, 255, 0, 255, 0, 0, //
        0, 255, 255, 255, 0, 0, //
        0, 0, 255, 255, 0, 0,
    ])
}

const FREE_FORM: &str = "/ShadingType 4 /ColorSpace /DeviceRGB /BitsPerCoordinate 8 \
     /BitsPerComponent 8 /BitsPerFlag 8 /Decode [0 100 0 100 0 1 0 1 0 1] \
     /Filter /ASCIIHexDecode";

/// A red Coons patch covering the page: flag, 12 boundary points, then an
/// RGB colour per corner.
fn coons() -> String {
    let pts: [(u8, u8); 12] = [
        (0, 0),
        (0, 85),
        (0, 170),
        (0, 255),
        (85, 255),
        (170, 255),
        (255, 255),
        (255, 170),
        (255, 85),
        (255, 0),
        (170, 0),
        (85, 0),
    ];
    let mut data = vec![0u8];
    for (x, y) in pts {
        data.extend([x, y]);
    }
    for _ in 0..4 {
        data.extend([255, 0, 0]);
    }
    hex(&data)
}

const COONS: &str = "/ShadingType 6 /ColorSpace /DeviceRGB /BitsPerCoordinate 8 \
     /BitsPerComponent 8 /BitsPerFlag 8 /Decode [0 100 0 100 0 1 0 1 0 1] \
     /Filter /ASCIIHexDecode";

/// A page painting the mesh `dict`/`data` as `/Sh0` (object 5) with
/// `content`. `/GS1` sets a luminosity soft mask whose group (object 6)
/// paints `mask`.
fn mesh_page(dict: &str, data: &str, content: &str, mask: &str) -> justpdf_core::PdfDocument {
    let resources = "<< /Shading << /Sh0 5 0 R >> \
         /ExtGState << /GS1 << /SMask << /S /Luminosity /G 6 0 R >> >> >> >>";
    let group = "/Type /XObject /Subtype /Form /BBox [0 0 100 100] \
         /Group << /S /Transparency /CS /DeviceGray >>";
    let objects: Vec<Obj<'_>> = vec![Stream(dict, data), Stream(group, mask)];
    page(resources, content, objects)
}

/// Within 3 per channel: the mesh rasterizer truncates (255 → 254).
fn near(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(&x, y)| x.abs_diff(y) <= 3)
}

#[test]
fn a_free_form_mesh_is_clipped() {
    let doc = mesh_page(FREE_FORM, &free_form(), "0 0 50 100 re W n /Sh0 sh", "");
    let px = rgb(&doc, 25, 50);
    assert!(near(px, RED), "{px:?}");
    assert_eq!(rgb(&doc, 75, 50), WHITE);
}

#[test]
fn a_patch_mesh_is_clipped() {
    let doc = mesh_page(COONS, &coons(), "0 0 50 100 re W n /Sh0 sh", "");
    let px = rgb(&doc, 25, 50);
    assert!(near(px, RED), "{px:?}");
    assert_eq!(rgb(&doc, 75, 50), WHITE);
}

#[test]
fn a_free_form_mesh_is_masked_by_the_soft_mask() {
    let doc = mesh_page(
        FREE_FORM,
        &free_form(),
        "/GS1 gs /Sh0 sh",
        "1 g 0 0 50 100 re f",
    );
    let px = rgb(&doc, 25, 50);
    assert!(near(px, RED), "{px:?}");
    assert_eq!(rgb(&doc, 75, 50), WHITE);
}

#[test]
fn a_patch_mesh_is_masked_by_the_soft_mask() {
    let doc = mesh_page(COONS, &coons(), "/GS1 gs /Sh0 sh", "1 g 0 0 50 100 re f");
    let px = rgb(&doc, 25, 50);
    assert!(near(px, RED), "{px:?}");
    assert_eq!(rgb(&doc, 75, 50), WHITE);
}

#[test]
fn a_partial_soft_mask_blends_the_mesh_over_the_backdrop() {
    // Half coverage everywhere: red over white comes out (255, 128, 128).
    // (50, 50) lies on the edge the two triangles share, so it is painted once.
    let doc = mesh_page(
        FREE_FORM,
        &free_form(),
        "/GS1 gs /Sh0 sh",
        "0.5 g 0 0 100 100 re f",
    );
    let px = rgb(&doc, 50, 50);
    assert!(near(px, [255, 128, 128]), "{px:?}");
}

#[test]
fn a_mesh_outside_the_clip_keeps_what_was_painted_there() {
    // Blue under the right half survives a mesh clipped to the left half.
    let doc = mesh_page(
        FREE_FORM,
        &free_form(),
        "0 0 1 rg 50 0 50 100 re f q 0 0 50 100 re W n /Sh0 sh Q",
        "",
    );
    let px = rgb(&doc, 25, 50);
    assert!(near(px, RED), "{px:?}");
    assert_eq!(rgb(&doc, 75, 50), [0, 0, 255]);
}

#[test]
fn a_pixel_on_an_edge_two_triangles_share_is_painted() {
    // (50, 50) lies on the shared diagonal; one of the two triangles owns it.
    let doc = mesh_page(FREE_FORM, &free_form(), "/Sh0 sh", "");
    let px = rgb(&doc, 50, 50);
    assert!(near(px, RED), "{px:?}");
}

#[test]
fn a_shared_edge_is_painted_once_whatever_the_triangles_winding() {
    // The second triangle is given clockwise, the first anticlockwise.
    let data = hex(&[
        0, 0, 0, 255, 0, 0, //
        0, 255, 0, 255, 0, 0, //
        0, 0, 255, 255, 0, 0, //
        0, 255, 0, 255, 0, 0, //
        0, 0, 255, 255, 0, 0, //
        0, 255, 255, 255, 0, 0,
    ]);
    let doc = mesh_page(
        FREE_FORM,
        &data,
        "/GS1 gs /Sh0 sh",
        "0.5 g 0 0 100 100 re f",
    );
    let px = rgb(&doc, 50, 50);
    assert!(near(px, [255, 128, 128]), "{px:?}");
}
