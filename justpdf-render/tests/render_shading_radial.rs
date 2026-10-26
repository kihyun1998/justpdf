//! A radial shading paints, at each point, the colour of the largest `s`
//! whose circle passes through it, among those `/Extend` allows and whose
//! radius is not negative; the start radius counts (#389).

mod common;

use common::{RED, WHITE, page, rgb};

const BLUE: [u8; 3] = [0, 0, 255];

/// A page painting a red-to-blue radial shading with `coords` and `extend`
/// (dictionary text, possibly empty), after `before`.
fn radial(coords: &str, extend: &str, before: &str) -> justpdf_core::PdfDocument {
    let shading = format!(
        "<< /ShadingType 3 /ColorSpace /DeviceRGB /Coords [{coords}] \
         /Function << /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >> \
         {extend} >>"
    );
    page(
        &format!("<< /Shading << /S {shading} >> >>"),
        &format!("{before} /S sh"),
        vec![],
    )
}

/// Within 3 per channel of `want`.
fn near(got: [u8; 3], want: [u8; 3]) -> bool {
    got.iter().zip(want).all(|(&g, w)| g.abs_diff(w) <= 3)
}

/// Red to blue at `s`.
fn at_s(s: f64) -> [u8; 3] {
    [
        (255.0 * (1.0 - s)).round() as u8,
        0,
        (255.0 * s).round() as u8,
    ]
}

const RING: &str = "50 50 10 50 50 30";

/// `s` of `RING` at the centre of device pixel (x, y): its distance from
/// (50, 50) in user space, from radius 10 at 0 to 30 at 1.
fn ring_s(x: u32, y: u32) -> f64 {
    let (ux, uy) = (x as f64 + 0.5, 100.0 - (y as f64 + 0.5));
    (((ux - 50.0).powi(2) + (uy - 50.0).powi(2)).sqrt() - 10.0) / 20.0
}

#[test]
fn a_start_radius_starts_the_colours_at_the_start_circle() {
    let doc = radial(RING, "", "");
    for x in [62, 65, 75, 78] {
        let px = rgb(&doc, x, 50);
        assert!(near(px, at_s(ring_s(x, 50))), "x = {x}: {px:?}");
    }
}

#[test]
fn inside_the_start_circle_is_unpainted_without_extend() {
    let doc = radial(RING, "", "");
    assert_eq!(rgb(&doc, 50, 50), WHITE);
    assert_eq!(rgb(&doc, 55, 50), WHITE);
    assert_eq!(rgb(&doc, 85, 50), WHITE);
}

#[test]
fn extending_the_start_fills_the_start_circle() {
    let doc = radial(RING, "/Extend [true false]", "");
    assert_eq!(rgb(&doc, 50, 50), RED);
    assert_eq!(rgb(&doc, 85, 50), WHITE);
}

#[test]
fn extending_the_end_paints_past_the_end_circle() {
    let doc = radial(RING, "/Extend [false true]", "");
    assert_eq!(rgb(&doc, 85, 50), BLUE);
    assert_eq!(rgb(&doc, 50, 50), WHITE);
}

#[test]
fn overlapping_circles_take_the_largest_s_in_range() {
    // Start (30, 50) r 10, end (70, 50) r 20: x = 65 lies on the circles for
    // s = 0.5 and s = 1.5; without extend only 0.5 is in range.
    let doc = radial("30 50 10 70 50 20", "", "");
    let px = rgb(&doc, 65, 50);
    assert!(near(px, at_s(0.5)), "{px:?}");
    assert_eq!(rgb(&doc, 10, 50), WHITE);
}

#[test]
fn extending_both_ends_reaches_behind_the_start() {
    // x = 10 lies on the circles for s = -1/3 and -0.6: before the start.
    let doc = radial("30 50 10 70 50 20", "/Extend [true true]", "");
    assert_eq!(rgb(&doc, 10, 50), RED);
    let px = rgb(&doc, 65, 50);
    assert_eq!(px, BLUE, "s = 1.5 is in range once the end is extended");
}

#[test]
fn a_clipped_radial_shading_paints_only_inside_the_clip() {
    let doc = radial(RING, "", "0 0 70 100 re W n");
    let px = rgb(&doc, 65, 50);
    assert!(near(px, at_s(ring_s(65, 50))), "{px:?}");
    assert_eq!(rgb(&doc, 75, 50), WHITE);
}

#[test]
fn a_circle_of_negative_radius_paints_nothing() {
    // Shrinking from radius 30 to 10, extended past the end: a point at
    // distance d lies on s = (30 - d) / 20 and on s = (30 + d) / 20, whose
    // radius would be -d. The first is the one painted.
    let doc = radial("50 50 30 50 50 10", "/Extend [false true]", "");
    let (ux, uy) = (75.5_f64, 49.5_f64);
    let d = ((ux - 50.0).powi(2) + (uy - 50.0).powi(2)).sqrt();
    let px = rgb(&doc, 75, 50);
    assert!(near(px, at_s((30.0 - d) / 20.0)), "{px:?}");
}
