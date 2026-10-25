//! Axial, radial and mesh shadings evaluate `/Function` through core
//! `PdfFunction` (#233): single functions and arrays of them, and meshes with
//! a function carry one parametric value `t` per vertex.

mod common;

use common::{Obj, Plain, RED, Stream, page, rgb};
use justpdf_core::writer::encode_flate;

const BLUE: [u8; 3] = [0, 0, 255];
const GREEN: [u8; 3] = [0, 255, 0];

fn hex(data: &[u8]) -> String {
    let mut s: String = data.iter().map(|b| format!("{b:02X}")).collect();
    s.push('>');
    s
}

fn near(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(&x, y)| x.abs_diff(y) <= 3)
}

/// A page painting `/Sh0` = `shading` (a dictionary body), with `extra`
/// objects from 5.
fn shaded(shading: &str, extra: Vec<Obj<'_>>) -> justpdf_core::PdfDocument {
    page(
        &format!("<< /Shading << /Sh0 {shading} >> >>"),
        "/Sh0 sh",
        extra,
    )
}

/// An axial shading left to right across the page with `/Function` `f`.
fn axial(f: &str) -> String {
    format!(
        "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [0 0 100 0] \
         /Function {f} /Extend [true true] >>"
    )
}

// ---- axial ----------------------------------------------------------------------

#[test]
fn an_array_of_one_output_functions() {
    let f = "[<< /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [0] /N 1 >> \
              << /FunctionType 2 /Domain [0 1] /C0 [1] /C1 [1] /N 1 >> \
              << /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [0] /N 1 >>]";
    let doc = shaded(&axial(f), vec![]);
    assert_eq!(rgb(&doc, 50, 50), GREEN);
}

#[test]
fn an_exponential_function_keeps_its_exponent() {
    // /N 2 at t ≈ 0.5: 0.25 × 255 ≈ 64, not the linear 128.
    let f = "<< /FunctionType 2 /Domain [0 1] /C0 [0 0 0] /C1 [1 1 1] /N 2 >>";
    let doc = shaded(&axial(f), vec![]);
    let px = rgb(&doc, 50, 50);
    assert!(near(px, [65, 65, 65]), "{px:?}");
}

#[test]
fn a_stitching_function_with_referenced_sub_functions() {
    let f = "<< /FunctionType 3 /Domain [0 1] /Bounds [0.5] /Encode [0 1 0 1] \
             /Functions [5 0 R 6 0 R] >>";
    let doc = shaded(
        &axial(f),
        vec![
            Plain("<< /FunctionType 2 /Domain [0 1] /C0 [0 0 1] /C1 [0 0 1] /N 1 >>"),
            Plain("<< /FunctionType 2 /Domain [0 1] /C0 [0 1 0] /C1 [0 1 0] /N 1 >>"),
        ],
    );
    assert_eq!(rgb(&doc, 25, 50), BLUE);
    assert_eq!(rgb(&doc, 75, 50), GREEN);
}

#[test]
fn a_stitching_function_honours_encode() {
    // /Encode [1 0] runs the red → blue sub-function backwards.
    let f = "<< /FunctionType 3 /Domain [0 1] /Bounds [] /Encode [1 0] \
             /Functions [<< /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >>] >>";
    let doc = shaded(&axial(f), vec![]);
    let (left, right) = (rgb(&doc, 2, 50), rgb(&doc, 98, 50));
    assert!(left[2] > 240 && left[0] < 15, "{left:?}");
    assert!(right[0] > 240 && right[2] < 15, "{right:?}");
}

#[test]
fn an_axial_shading_with_a_sampled_function() {
    let doc = shaded(
        &axial("5 0 R"),
        vec![Stream(
            "/FunctionType 0 /Domain [0 1] /Range [0 1 0 1 0 1] /Size [2] /BitsPerSample 8 \
             /Filter /ASCIIHexDecode",
            "FF00000000FF>",
        )],
    );
    let (left, right) = (rgb(&doc, 2, 50), rgb(&doc, 98, 50));
    assert!(left[0] > 240 && left[2] < 15, "{left:?}");
    assert!(right[2] > 240 && right[0] < 15, "{right:?}");
}

#[test]
fn an_axial_shading_with_a_flate_postscript_function() {
    let code = hex(&encode_flate(b"{ pop 0 1 0 }").unwrap());
    let doc = shaded(
        &axial("5 0 R"),
        vec![Stream(
            "/FunctionType 4 /Domain [0 1] /Range [0 1 0 1 0 1] \
             /Filter [/ASCIIHexDecode /FlateDecode]",
            &code,
        )],
    );
    assert_eq!(rgb(&doc, 50, 50), GREEN);
}

// ---- radial ---------------------------------------------------------------------

#[test]
fn a_radial_shading_with_a_stitching_function() {
    // Blue for t < 0.5 (near the centre), green beyond.
    let shading = "<< /ShadingType 3 /ColorSpace /DeviceRGB /Coords [50 50 0 50 50 50] \
                   /Function << /FunctionType 3 /Domain [0 1] /Bounds [0.5] /Encode [0 1 0 1] \
                   /Functions [5 0 R 6 0 R] >> /Extend [true true] >>";
    let doc = shaded(
        shading,
        vec![
            Plain("<< /FunctionType 2 /Domain [0 1] /C0 [0 0 1] /C1 [0 0 1] /N 1 >>"),
            Plain("<< /FunctionType 2 /Domain [0 1] /C0 [0 1 0] /C1 [0 1 0] /N 1 >>"),
        ],
    );
    assert_eq!(rgb(&doc, 50, 50), BLUE);
    assert_eq!(rgb(&doc, 90, 50), GREEN);
}

// ---- meshes ---------------------------------------------------------------------

/// t → red: `t = 1` is pure red.
const T_TO_RED: &str = "<< /FunctionType 2 /Domain [0 1] /C0 [0 0 0] /C1 [1 0 0] /N 1 >>";

#[test]
fn a_free_form_mesh_maps_t_through_its_function() {
    // One triangle (0,0) (100,0) (0,100), t = 1 at each vertex: flag, x, y, t.
    let data = hex(&[0, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255]);
    let dict = format!(
        "/ShadingType 4 /ColorSpace /DeviceRGB /BitsPerCoordinate 8 /BitsPerComponent 8 \
         /BitsPerFlag 8 /Decode [0 100 0 100 0 1] /Function {T_TO_RED} /Filter /ASCIIHexDecode"
    );
    let doc = page(
        "<< /Shading << /Sh0 5 0 R >> >>",
        "/Sh0 sh",
        vec![Stream(&dict, &data)],
    );
    assert_eq!(rgb(&doc, 20, 80), RED);
}

#[test]
fn a_free_form_mesh_interpolates_t_before_the_function() {
    // t = 0 at (0,0), 1 at the other two corners; the function squares t
    // (/N 2), so midway along the bottom edge (t ≈ 0.5) red is ≈ 0.25, where
    // interpolating the colours would give ≈ 0.5.
    let data = hex(&[0, 0, 0, 0, 0, 255, 0, 255, 0, 0, 255, 255]);
    let f = "<< /FunctionType 2 /Domain [0 1] /C0 [0 0 0] /C1 [1 0 0] /N 2 >>";
    let dict = format!(
        "/ShadingType 4 /ColorSpace /DeviceRGB /BitsPerCoordinate 8 /BitsPerComponent 8 \
         /BitsPerFlag 8 /Decode [0 100 0 100 0 1] /Function {f} /Filter /ASCIIHexDecode"
    );
    let doc = page(
        "<< /Shading << /Sh0 5 0 R >> >>",
        "/Sh0 sh",
        vec![Stream(&dict, &data)],
    );
    let px = rgb(&doc, 50, 98);
    assert!(px[0] > 45 && px[0] < 85, "{px:?}");
}

#[test]
fn a_lattice_mesh_maps_t_through_its_function() {
    // 2 × 2 lattice over the page: x, y, t per vertex, t = 1.
    let data = hex(&[0, 0, 255, 255, 0, 255, 0, 255, 255, 255, 255, 255]);
    let dict = format!(
        "/ShadingType 5 /ColorSpace /DeviceRGB /BitsPerCoordinate 8 /BitsPerComponent 8 \
         /VerticesPerRow 2 /Decode [0 100 0 100 0 1] /Function {T_TO_RED} /Filter /ASCIIHexDecode"
    );
    let doc = page(
        "<< /Shading << /Sh0 5 0 R >> >>",
        "/Sh0 sh",
        vec![Stream(&dict, &data)],
    );
    assert_eq!(rgb(&doc, 50, 50), RED);
}

#[test]
fn a_coons_patch_maps_t_through_its_function() {
    // One patch with flag 0: 12 boundary points around the page square, then
    // four t values of 1.
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
    data.extend([255, 255, 255, 255]);
    let dict = format!(
        "/ShadingType 6 /ColorSpace /DeviceRGB /BitsPerCoordinate 8 /BitsPerComponent 8 \
         /BitsPerFlag 8 /Decode [0 100 0 100 0 1] /Function {T_TO_RED} /Filter /ASCIIHexDecode"
    );
    let doc = page(
        "<< /Shading << /Sh0 5 0 R >> >>",
        "/Sh0 sh",
        vec![Stream(&dict, &hex(&data))],
    );
    assert_eq!(rgb(&doc, 50, 50), RED);
}

#[test]
fn a_mesh_without_a_function_keeps_its_colours() {
    // The same triangle with an RGB colour per vertex (blue).
    let data = hex(&[
        0, 0, 0, 0, 0, 255, 0, 255, 0, 0, 0, 255, 0, 0, 255, 0, 0, 255,
    ]);
    let dict = "/ShadingType 4 /ColorSpace /DeviceRGB /BitsPerCoordinate 8 /BitsPerComponent 8 \
                /BitsPerFlag 8 /Decode [0 100 0 100 0 1 0 1 0 1] /Filter /ASCIIHexDecode";
    let doc = page(
        "<< /Shading << /Sh0 5 0 R >> >>",
        "/Sh0 sh",
        vec![Stream(dict, &data)],
    );
    // Colour interpolation truncates, as it always has: within ±3.
    let px = rgb(&doc, 20, 80);
    assert!(near(px, BLUE), "{px:?}");
}
