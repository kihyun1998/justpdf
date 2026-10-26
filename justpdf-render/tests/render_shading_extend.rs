//! Axial and radial shadings paint beyond their start and end only where
//! `/Extend` says so (#384).

mod common;

use common::{RED, WHITE, page, rgb};

const BLUE: [u8; 3] = [0, 0, 255];

/// A page painting `/Sh0` = `shading` (a dictionary body).
fn shaded(shading: &str) -> justpdf_core::PdfDocument {
    page(
        &format!("<< /Shading << /Sh0 {shading} >> >>"),
        "/Sh0 sh",
        vec![],
    )
}

/// Red at t = 0 to blue at t = 1.
const RED_TO_BLUE: &str = "<< /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >>";

/// An axial shading from x = 25 to x = 75, red to blue, with `extend`
/// (dictionary text, possibly empty).
fn axial(extend: &str) -> justpdf_core::PdfDocument {
    shaded(&format!(
        "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [25 0 75 0] \
         /Function {RED_TO_BLUE} {extend} >>"
    ))
}

// ---- axial ----------------------------------------------------------------------

#[test]
fn an_axial_shading_without_extend_paints_only_between_its_ends() {
    let doc = axial("");
    assert_eq!(rgb(&doc, 10, 50), WHITE);
    assert_eq!(rgb(&doc, 90, 50), WHITE);
    let px = rgb(&doc, 50, 50);
    assert!(px[0] > 100 && px[2] > 100, "{px:?}");
}

#[test]
fn an_axial_shading_extends_its_start_only() {
    let doc = axial("/Extend [true false]");
    assert_eq!(rgb(&doc, 10, 50), RED);
    assert_eq!(rgb(&doc, 90, 50), WHITE);
}

#[test]
fn an_axial_shading_extends_its_end_only() {
    let doc = axial("/Extend [false true]");
    assert_eq!(rgb(&doc, 10, 50), WHITE);
    assert_eq!(rgb(&doc, 90, 50), BLUE);
}

#[test]
fn a_slanted_axial_shading_cuts_across_its_axis() {
    // Axis from (25, 25) to (75, 75) in user space (y up); the band's edges
    // are perpendicular to it. Device (60, 85) is user (60, 15): t = 0.25,
    // inside the band although x and y alone lie outside [25 75] on one axis.
    let doc = shaded(&format!(
        "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [25 25 75 75] \
         /Function {RED_TO_BLUE} >>"
    ));
    assert_ne!(rgb(&doc, 60, 85), WHITE);
    // User (20, 40): t = 0.1, left of x = 25.
    assert_ne!(rgb(&doc, 20, 60), WHITE);
    // User (30, 2): t < 0, right of x = 25.
    assert_eq!(rgb(&doc, 30, 98), WHITE);
    // User (10, 10): t < 0.
    assert_eq!(rgb(&doc, 10, 90), WHITE);
    // User (90, 90): t > 1.
    assert_eq!(rgb(&doc, 90, 10), WHITE);
}

// ---- radial ---------------------------------------------------------------------

/// A red radial shading with `coords`, with `extend`.
fn radial(coords: &str, extend: &str) -> justpdf_core::PdfDocument {
    shaded(&format!(
        "<< /ShadingType 3 /ColorSpace /DeviceRGB /Coords [{coords}] \
         /Function << /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [1 0 0] /N 1 >> \
         {extend} >>"
    ))
}

#[test]
fn a_radial_shading_without_extend_stops_at_its_end_circle() {
    let doc = radial("50 50 0 50 50 25", "");
    assert_eq!(rgb(&doc, 50, 50), RED);
    assert_eq!(rgb(&doc, 70, 50), RED);
    assert_eq!(rgb(&doc, 90, 50), WHITE);
}

#[test]
fn a_radial_shading_extends_beyond_its_end_circle() {
    let doc = radial("50 50 0 50 50 25", "/Extend [false true]");
    assert_eq!(rgb(&doc, 90, 50), RED);
}

#[test]
fn a_radial_shading_with_an_outside_start_paints_the_cone_between_its_circles() {
    // Start point (10, 50), end circle at (70, 50) radius 20: the painted
    // region is their convex hull.
    let doc = radial("10 50 0 70 50 20", "");
    assert_eq!(rgb(&doc, 30, 50), RED);
    assert_eq!(rgb(&doc, 30, 30), WHITE);
    assert_eq!(rgb(&doc, 95, 50), WHITE);
}
