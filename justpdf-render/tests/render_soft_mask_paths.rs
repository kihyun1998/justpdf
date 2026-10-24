//! Everything painted while a soft mask is in the graphics state is masked by
//! it (ISO 32000-2 §11.6.4.3), except an image XObject with a mask of its
//! own; `/SMask` and `/Group` are read through indirect references (#129).

use justpdf_core::PdfDocument;
use justpdf_render::{RenderOptions, render_page_to_pixmap};

/// A PDF whose object `n` (1-based) is `objects[n - 1]`, each a dictionary
/// (or other object) and an optional stream body.
fn pdf(objects: Vec<(String, Option<String>)>) -> PdfDocument {
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, (dict, stream)) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        match stream {
            Some(data) => {
                out.extend_from_slice(
                    format!("<< {dict} /Length {} >>\nstream\n", data.len()).as_bytes(),
                );
                out.extend_from_slice(data.as_bytes());
                out.extend_from_slice(b"\nendstream");
            }
            None => out.extend_from_slice(dict.as_bytes()),
        }
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref_at = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    PdfDocument::from_bytes(out).unwrap()
}

/// A 200 × 200 page with `content` and `resources`. Object 5 is the soft
/// mask form, black everywhere and white on the right half, named by `/GS1`
/// (direct) and `/GS2` (through the soft-mask dictionary at object 6);
/// `/M` sets the Multiply blend mode. `extra` follows as objects 7, 8, ….
fn page(content: &str, resources: &str, extra: Vec<(String, Option<&str>)>) -> PdfDocument {
    let mut objects = vec![
        ("<< /Type /Catalog /Pages 2 0 R >>".to_string(), None),
        (
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            None,
        ),
        (
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << \
                 /ExtGState << /GS1 << /SMask << /S /Luminosity /G 5 0 R >> >> \
                 /GS2 << /SMask 6 0 R >> /M << /BM /Multiply >> >> {resources} >> /Contents 4 0 R >>"
            ),
            None,
        ),
        (String::new(), Some(content.to_string())),
        (
            "/Type /XObject /Subtype /Form /BBox [0 0 200 200] \
             /Group << /S /Transparency /CS /DeviceGray >>"
                .to_string(),
            Some("0 g 0 0 200 200 re f 1 g 100 0 100 200 re f".to_string()),
        ),
        ("<< /S /Luminosity /G 5 0 R >>".to_string(), None),
    ];
    for (dict, body) in extra {
        objects.push((dict, body.map(str::to_string)));
    }
    pdf(objects)
}

fn pixmap(doc: &PdfDocument) -> justpdf_render::RenderedPixmap {
    render_page_to_pixmap(doc, 0, &RenderOptions::default()).unwrap()
}

/// The RGB of the pixel at page point (`x`, `y`), a pixel per point.
fn rgb(doc: &PdfDocument, x: u32, y: u32) -> [u8; 3] {
    let px = pixmap(doc);
    let i = (((px.height - 1 - y) * px.width + x) * 4) as usize;
    [px.data[i], px.data[i + 1], px.data[i + 2]]
}

/// Red pixels in the left half and in the right half of the page.
fn red_halves(doc: &PdfDocument) -> (usize, usize) {
    let px = pixmap(doc);
    let (mut left, mut right) = (0, 0);
    for y in 0..px.height {
        for x in 0..px.width {
            let i = ((y * px.width + x) * 4) as usize;
            if px.data[i] > 200 && px.data[i + 1] < 60 && px.data[i + 2] < 60 {
                if x < px.width / 2 {
                    left += 1;
                } else {
                    right += 1;
                }
            }
        }
    }
    (left, right)
}

const RED: [u8; 3] = [255, 0, 0];
const WHITE: [u8; 3] = [255, 255, 255];

/// A red tiling pattern, object 7.
fn red_pattern() -> (String, Option<&'static str>) {
    (
        "/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 \
         /BBox [0 0 10 10] /XStep 10 /YStep 10 /Resources << >>"
            .to_string(),
        Some("1 0 0 rg 0 0 10 10 re f"),
    )
}

fn assert_masked(doc: &PdfDocument) {
    assert_eq!(rgb(doc, 25, 100), WHITE, "masked out on the left");
    assert_eq!(rgb(doc, 175, 100), RED, "painted on the right");
}

#[test]
fn a_pattern_fill_is_masked() {
    let doc = page(
        "/GS1 gs /Pattern cs /P0 scn 0 0 200 200 re f",
        "/Pattern << /P0 7 0 R >>",
        vec![red_pattern()],
    );
    assert_masked(&doc);
}

#[test]
fn a_pattern_stroke_is_masked() {
    let doc = page(
        "/GS1 gs /Pattern CS /P0 SCN 40 w 0 100 m 200 100 l S",
        "/Pattern << /P0 7 0 R >>",
        vec![red_pattern()],
    );
    assert_masked(&doc);
}

#[test]
fn a_shading_pattern_fill_is_masked_once() {
    // A uniform 0.5 mask over a red shading pattern: half-red, as a solid
    // fill under the same mask gives, not quarter-red.
    let doc = |fill: &str| {
        pdf(vec![
            ("<< /Type /Catalog /Pages 2 0 R >>".into(), None),
            ("<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(), None),
            (
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << \
                 /ExtGState << /GS1 << /SMask << /S /Luminosity /G 5 0 R >> >> >> \
                 /Pattern << /P0 6 0 R >> >> /Contents 4 0 R >>"
                    .into(),
                None,
            ),
            (
                String::new(),
                Some(format!("/GS1 gs {fill} 0 0 200 200 re f")),
            ),
            (
                "/Type /XObject /Subtype /Form /BBox [0 0 200 200] \
                 /Group << /S /Transparency /CS /DeviceGray >>"
                    .into(),
                Some("0.5 g 0 0 200 200 re f".into()),
            ),
            (
                "<< /Type /Pattern /PatternType 2 /Shading << /ShadingType 2 \
                 /ColorSpace /DeviceRGB /Coords [0 0 200 0] /Function << /FunctionType 2 \
                 /Domain [0 1] /C0 [1 0 0] /C1 [1 0 0] /N 1 >> >> >>"
                    .into(),
                None,
            ),
        ])
    };
    let shaded = rgb(&doc("/Pattern cs /P0 scn"), 100, 100);
    let solid = rgb(&doc("1 0 0 rg"), 100, 100);
    assert_eq!(shaded, solid);
    assert!(
        shaded[1] > 100 && shaded[1] < 160,
        "half-masked red: {shaded:?}"
    );
}

#[test]
fn sh_is_masked() {
    let doc = page(
        "/GS1 gs /Sh0 sh",
        "/Shading << /Sh0 << /ShadingType 2 /ColorSpace /DeviceRGB /Coords [0 0 200 0] \
         /Function << /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [1 0 0] /N 1 >> >> >>",
        vec![],
    );
    assert_masked(&doc);
}

#[test]
fn text_is_masked() {
    let content = "/GS1 gs 1 0 0 rg BT /F1 80 Tf 0 70 Td (MMM) Tj ET";
    let resources = "/Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >>";
    let (left, right) = red_halves(&page(content, resources, vec![]));
    assert_eq!(left, 0, "no text on the masked-out left");
    assert!(right > 500, "text on the right: {right}");
}

#[test]
fn a_stencil_image_mask_is_masked() {
    let doc = page(
        "/GS1 gs 1 0 0 rg 200 0 0 200 0 0 cm /Im0 Do",
        "/XObject << /Im0 7 0 R >>",
        vec![(
            "/Type /XObject /Subtype /Image /ImageMask true /Width 1 /Height 1 \
             /BitsPerComponent 1 /Filter /ASCIIHexDecode"
                .to_string(),
            Some("00>"),
        )],
    );
    assert_masked(&doc);
}

#[test]
fn an_image_with_its_own_smask_ignores_the_soft_mask() {
    // An opaque red image whose /SMask is all 255: the image's mask overrides
    // the graphics state's soft mask, so the left half is red too.
    let doc = page(
        "/GS1 gs 200 0 0 200 0 0 cm /Im0 Do",
        "/XObject << /Im0 7 0 R >>",
        vec![
            (
                "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceRGB \
                 /BitsPerComponent 8 /SMask 8 0 R /Filter /ASCIIHexDecode"
                    .to_string(),
                Some("FF0000>"),
            ),
            (
                "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray \
                 /BitsPerComponent 8 /Filter /ASCIIHexDecode"
                    .to_string(),
                Some("FF>"),
            ),
        ],
    );
    assert_eq!(rgb(&doc, 25, 100), RED);
    assert_eq!(rgb(&doc, 175, 100), RED);
}

#[test]
fn an_image_without_a_mask_is_masked() {
    let doc = page(
        "/GS1 gs 200 0 0 200 0 0 cm /Im0 Do",
        "/XObject << /Im0 7 0 R >>",
        vec![(
            "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceRGB \
             /BitsPerComponent 8 /Filter /ASCIIHexDecode"
                .to_string(),
            Some("FF0000>"),
        )],
    );
    assert_masked(&doc);
}

#[test]
fn an_indirect_smask_is_applied() {
    let doc = page("/GS2 gs 1 0 0 rg 0 0 200 200 re f", "", vec![]);
    assert_masked(&doc);
}

/// A black left half, then a non-isolated group composited with Multiply that
/// fills `0.9 0.9 0.8` with Normal inside. `group` is how the form writes its
/// `/Group` (object 9 holds the dictionary). The pixel at (25, 100).
fn multiplied_over_black(group: &str) -> [u8; 3] {
    let doc = page(
        "0 g 0 0 100 200 re f /M gs /G0 Do",
        "/XObject << /G0 7 0 R >>",
        vec![
            (
                format!(
                    "/Type /XObject /Subtype /Form /BBox [0 0 200 200] /Group {group} \
                     /Resources << /ExtGState << /N 8 0 R >> >>"
                ),
                Some("/N gs 0.9 0.9 0.8 rg 0 0 200 200 re f"),
            ),
            ("<< /BM /Normal >>".to_string(), None),
            ("<< /S /Transparency /I false /K false >>".to_string(), None),
        ],
    );
    rgb(&doc, 25, 100)
}

#[test]
fn a_direct_transparency_group_is_composited() {
    let group = "<< /S /Transparency /I false /K false >>";
    assert_eq!(multiplied_over_black(group), [0, 0, 0]);
}

#[test]
fn an_indirect_transparency_group_is_composited() {
    assert_eq!(multiplied_over_black("9 0 R"), [0, 0, 0]);
}

#[test]
fn a_placeholder_glyph_is_masked() {
    // A Type3 font has no glyph program the renderer draws (#226), so each
    // glyph is drawn as a placeholder rectangle.
    let content = "/GS1 gs 1 0 0 rg BT /F3 80 Tf 0 70 Td (MMM) Tj ET";
    let resources = "/Font << /F3 << /Type /Font /Subtype /Type3 /FontBBox [0 0 1000 1000] \
                     /FontMatrix [0.001 0 0 0.001 0 0] /CharProcs << >> \
                     /Encoding << /Differences [77 /M] >> /FirstChar 77 /LastChar 77 \
                     /Widths [800] /Resources << >> >> >>";
    let (left, right) = red_halves(&page(content, resources, vec![]));
    assert_eq!(left, 0, "no placeholder on the masked-out left");
    assert!(right > 500, "placeholders on the right: {right}");
}

/// A page with two soft masks: `/L` white on the left half, `/R` white on
/// the right half.
fn two_mask_page(content: &str) -> PdfDocument {
    let form = |body: &str| {
        (
            "/Type /XObject /Subtype /Form /BBox [0 0 200 200] \
             /Group << /S /Transparency /CS /DeviceGray >>"
                .to_string(),
            Some(body.to_string()),
        )
    };
    pdf(vec![
        ("<< /Type /Catalog /Pages 2 0 R >>".into(), None),
        ("<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(), None),
        (
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << \
             /ExtGState << /L << /SMask << /S /Luminosity /G 5 0 R >> >> \
             /R << /SMask << /S /Luminosity /G 6 0 R >> >> >> >> /Contents 4 0 R >>"
                .into(),
            None,
        ),
        (String::new(), Some(content.to_string())),
        form("0 g 0 0 200 200 re f 1 g 0 0 100 200 re f"),
        form("0 g 0 0 200 200 re f 1 g 100 0 100 200 re f"),
    ])
}

#[test]
fn a_new_clip_under_the_same_soft_mask_is_used() {
    // Under /R: a red fill in the full clip, then a blue fill in the clip
    // [150 0 50 200]. Blue shows only from x = 150.
    let doc = two_mask_page(
        "0 0 200 200 re W n /R gs 1 0 0 rg 0 0 200 200 re f \
         q 150 0 50 200 re W n 0 0 1 rg 0 0 200 200 re f Q",
    );
    assert_eq!(rgb(&doc, 125, 100), RED);
    assert_eq!(rgb(&doc, 175, 100), [0, 0, 255]);
}

#[test]
fn a_new_soft_mask_under_the_same_clip_is_used() {
    // In one clip: a red fill under /L, then a blue fill under /R. Each
    // mask is set in its own q … Q, so /R's mask form does not run under /L
    // (#128).
    let doc = two_mask_page(
        "0 0 200 200 re W n q /L gs 1 0 0 rg 0 0 200 200 re f Q \
         q /R gs 0 0 1 rg 0 0 200 200 re f Q",
    );
    assert_eq!(rgb(&doc, 25, 100), RED);
    assert_eq!(rgb(&doc, 175, 100), [0, 0, 255]);
}
