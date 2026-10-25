//! Fill and stroke colours and shadings in resource colour spaces: `cs`/`CS`
//! look a name up in the current `/Resources /ColorSpace`, set the space's
//! initial colour, and `sc`/`scn` components and shading output convert
//! through the core colour model (#231).

mod common;

use common::{Obj, Plain, RED, Stream, page, rgb};
use justpdf_render::render_page_to_svg;

const BLUE: [u8; 3] = [0, 0, 255];
const BLACK: [u8; 3] = [0, 0, 0];

/// A page whose `/CS0` colour space is `cs`, drawing `content`.
fn with_cs0(cs: &str, content: &str) -> justpdf_core::PdfDocument {
    page(
        &format!("<< /ColorSpace << /CS0 {cs} >> >>"),
        content,
        vec![],
    )
}

const INDEXED: &str = "[/Indexed /DeviceRGB 1 <FF00000000FF>]";
/// Tint t → CMYK `0 t t 0`: red ink.
const SEPARATION: &str = "[/Separation /Spot /DeviceCMYK \
     << /FunctionType 2 /Domain [0 1] /C0 [0 0 0 0] /C1 [0 1 1 0] /N 1 >>]";
const DEVICE_N: &str = "[/DeviceN [/Spot] /DeviceCMYK \
     << /FunctionType 2 /Domain [0 1] /C0 [0 0 0 0] /C1 [0 1 1 0] /N 1 >>]";
const ICC_GRAY: &str = "[/ICCBased 5 0 R]";

fn centre(doc: &justpdf_core::PdfDocument) -> [u8; 3] {
    rgb(doc, 50, 50)
}

// ---- fill -------------------------------------------------------------------

#[test]
fn an_indexed_fill_looks_its_index_up() {
    let doc = with_cs0(INDEXED, "/CS0 cs 1 scn 0 0 100 100 re f");
    assert_eq!(centre(&doc), BLUE);
}

#[test]
fn a_separation_fill_goes_through_its_tint_transform() {
    // Tint 0.5 → CMYK 0 .5 .5 0 → (255, 127, 127).
    let doc = with_cs0(SEPARATION, "/CS0 cs 0.5 scn 0 0 100 100 re f");
    assert_eq!(centre(&doc), [255, 127, 127]);
}

#[test]
fn a_device_n_fill_goes_through_its_tint_transform() {
    let doc = with_cs0(DEVICE_N, "/CS0 cs 0.5 scn 0 0 100 100 re f");
    assert_eq!(centre(&doc), [255, 127, 127]);
}

#[test]
fn an_icc_based_fill_uses_its_component_count() {
    let doc = page(
        &format!("<< /ColorSpace << /CS0 {ICC_GRAY} >> >>"),
        "/CS0 cs 0.5 scn 0 0 100 100 re f",
        vec![Stream("/N 1 /Alternate /DeviceGray", "")],
    );
    assert_eq!(centre(&doc), [127, 127, 127]);
}

#[test]
fn a_device_rgb_resource_fills_as_rgb() {
    let doc = with_cs0("/DeviceRGB", "/CS0 cs 0 0 1 scn 0 0 100 100 re f");
    assert_eq!(centre(&doc), BLUE);
}

#[test]
fn an_indexed_stroke_looks_its_index_up() {
    let doc = with_cs0(INDEXED, "/CS0 CS 1 SCN 20 w 0 50 m 100 50 l S");
    assert_eq!(centre(&doc), BLUE);
}

// ---- the initial colour -----------------------------------------------------

#[test]
fn cs_sets_the_initial_colour() {
    // Index 0 is red; a Separation starts at tint 1, full red ink.
    let indexed = with_cs0(INDEXED, "0 0 1 rg /CS0 cs 0 0 100 100 re f");
    assert_eq!(centre(&indexed), RED);
    let separation = with_cs0(SEPARATION, "0 0 1 rg /CS0 cs 0 0 100 100 re f");
    assert_eq!(centre(&separation), RED);
    let gray = page("<< >>", "1 0 0 rg /DeviceGray cs 0 0 100 100 re f", vec![]);
    assert_eq!(centre(&gray), BLACK);
}

#[test]
fn a_pattern_fill_through_a_pattern_resource_space() {
    // `/CS0 cs` with a `[/Pattern]` resource, then `scn` selects the pattern.
    let doc = page(
        "<< /ColorSpace << /CS0 [/Pattern] >> /Pattern << /P0 5 0 R >> >>",
        "/CS0 cs /P0 scn 0 0 100 100 re f",
        vec![Stream(
            "/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 \
             /BBox [0 0 10 10] /XStep 10 /YStep 10 /Resources << >>",
            "1 0 0 rg 0 0 10 10 re f",
        )],
    );
    assert_eq!(centre(&doc), RED);
}

// ---- resource scope ---------------------------------------------------------

#[test]
fn a_colour_space_in_a_forms_own_resources_is_found() {
    let doc = page(
        "<< /XObject << /X0 5 0 R >> >>",
        "/X0 Do",
        vec![Stream(
            &format!(
                "/Type /XObject /Subtype /Form /BBox [0 0 100 100] \
                 /Resources << /ColorSpace << /CS0 {INDEXED} >> >>"
            ),
            "/CS0 cs 1 scn 0 0 100 100 re f",
        )],
    );
    assert_eq!(centre(&doc), BLUE);
}

// ---- SVG --------------------------------------------------------------------

fn svg_fill(doc: &justpdf_core::PdfDocument) -> String {
    let svg = render_page_to_svg(doc, 0).unwrap();
    let start = svg
        .find("fill=\"rgb(")
        .map(|i| i + 6)
        .expect("a filled path");
    svg[start..start + svg[start..].find('"').unwrap()].to_string()
}

#[test]
fn svg_an_indexed_fill_looks_its_index_up() {
    let doc = with_cs0(INDEXED, "/CS0 cs 1 scn 0 0 100 100 re f");
    assert_eq!(svg_fill(&doc), "rgb(0,0,255)");
}

#[test]
fn svg_a_device_rgb_resource_fills_as_rgb() {
    let doc = with_cs0("/DeviceRGB", "/CS0 cs 0 0 1 scn 0 0 100 100 re f");
    assert_eq!(svg_fill(&doc), "rgb(0,0,255)");
}

// ---- shadings ---------------------------------------------------------------

/// A page painting `/Sh0`, an axial shading left to right from function value
/// 0 to 1 in colour space `cs`, with `extra` objects from 5.
fn axial(cs: &str, extra: Vec<Obj<'_>>) -> justpdf_core::PdfDocument {
    page(
        &format!(
            "<< /Shading << /Sh0 << /ShadingType 2 /ColorSpace {cs} /Coords [0 0 100 0] \
             /Function << /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 1 >> \
             /Extend [true true] >> >> >>"
        ),
        "/Sh0 sh",
        extra,
    )
}

fn is_gray(c: [u8; 3]) -> bool {
    c[0] == c[1] && c[1] == c[2]
}

#[test]
fn a_gray_shading_is_unchanged() {
    let doc = axial("/DeviceGray", vec![]);
    let (left, right) = (rgb(&doc, 10, 50), rgb(&doc, 90, 50));
    assert!(is_gray(left) && is_gray(right), "{left:?} {right:?}");
    assert!(left[0] < 60 && right[0] > 200, "{left:?} {right:?}");
}

#[test]
fn a_separation_shading_goes_through_its_tint_transform() {
    let doc = axial(SEPARATION, vec![]);
    let (left, right) = (rgb(&doc, 10, 50), rgb(&doc, 90, 50));
    // Little red ink on the left, a lot on the right; red stays 255.
    assert!(left[0] > 240 && left[1] > 200, "{left:?}");
    assert!(right[0] > 240 && right[1] < 60, "{right:?}");
}

#[test]
fn an_icc_based_shading_uses_its_component_count() {
    let doc = axial(ICC_GRAY, vec![Stream("/N 1 /Alternate /DeviceGray", "")]);
    let (left, right) = (rgb(&doc, 10, 50), rgb(&doc, 90, 50));
    assert!(is_gray(left) && is_gray(right), "{left:?} {right:?}");
}

#[test]
fn a_shading_colour_space_by_reference() {
    let doc = axial("5 0 R", vec![Plain(SEPARATION)]);
    let right = rgb(&doc, 90, 50);
    assert!(right[0] > 240 && right[1] < 60, "{right:?}");
}

#[test]
fn a_shading_pattern_fill_goes_through_the_colour_space() {
    let doc = page(
        "<< /Pattern << /P0 << /Type /Pattern /PatternType 2 /Shading << /ShadingType 2 \
         /ColorSpace [/ICCBased 5 0 R] /Coords [0 0 100 0] \
         /Function << /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 1 >> \
         /Extend [true true] >> >> >> >>",
        "/Pattern cs /P0 scn 0 0 100 100 re f",
        vec![Stream("/N 1 /Alternate /DeviceGray", "")],
    );
    let (left, right) = (rgb(&doc, 10, 50), rgb(&doc, 90, 50));
    assert!(is_gray(left) && is_gray(right), "{left:?} {right:?}");
}
