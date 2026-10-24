//! The clipping path is part of the graphics state (ISO 32000-2 §8.5.4): `Q`
//! restores the clip saved by its `q`. The soft mask is a separate parameter
//! (§11.6.5): it masks each draw once and never changes the clip (#50).

use justpdf_core::PdfDocument;
use justpdf_render::{RenderOptions, render_page_to_pixmap};

/// A PDF whose object `n` (1-based) is `objects[n - 1]`, each a dictionary
/// and an optional stream body.
fn pdf(objects: Vec<(String, Option<Vec<u8>>)>) -> PdfDocument {
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
                out.extend_from_slice(data);
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

/// A 200 × 200 page with `content`, resources `resources`, and the extra
/// stream objects `streams` as objects 5, 6, ….
fn page(content: &str, resources: &str, streams: &[(&str, &str)]) -> PdfDocument {
    let mut objects = vec![
        ("<< /Type /Catalog /Pages 2 0 R >>".to_string(), None),
        (
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            None,
        ),
        (
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] \
                 /Resources {resources} /Contents 4 0 R >>"
            ),
            None,
        ),
        (String::new(), Some(content.as_bytes().to_vec())),
    ];
    for (dict, body) in streams {
        objects.push((dict.to_string(), Some(body.as_bytes().to_vec())));
    }
    pdf(objects)
}

/// A page whose `/GS1` sets a luminosity soft mask drawn by `mask_content`
/// (object 5), and whose `/GS0` sets `/SMask /None`.
fn soft_mask_page(content: &str, mask_content: &str) -> PdfDocument {
    page(
        content,
        "<< /ExtGState << /GS1 << /SMask << /S /Luminosity /G 5 0 R >> >> \
         /GS0 << /SMask /None >> >> >>",
        &[(
            "/Type /XObject /Subtype /Form /BBox [0 0 200 200] \
             /Group << /S /Transparency /CS /DeviceGray >>",
            mask_content,
        )],
    )
}

/// The RGB of the pixel at page point (`x`, `y`), a pixel per point.
fn rgb(doc: &PdfDocument, x: u32, y: u32) -> [u8; 3] {
    let px = render_page_to_pixmap(doc, 0, &RenderOptions::default()).unwrap();
    let i = (((px.height - 1 - y) * px.width + x) * 4) as usize;
    [px.data[i], px.data[i + 1], px.data[i + 2]]
}

const BLUE: [u8; 3] = [0, 0, 255];
const WHITE: [u8; 3] = [255, 255, 255];

#[test]
fn q_restores_the_outer_clip_after_a_nested_one() {
    let doc = page(
        "q 0 0 100 200 re W n q 0 0 50 200 re W n Q 0 0 1 rg 0 0 200 200 re f Q",
        "<< >>",
        &[],
    );
    assert_eq!(rgb(&doc, 75, 100), BLUE, "inside the outer clip only");
    assert_eq!(rgb(&doc, 150, 100), WHITE, "outside the outer clip");
}

#[test]
fn a_soft_mask_does_not_outlive_its_q() {
    let doc = soft_mask_page(
        "q /GS1 gs 1 0 0 rg 0 0 200 200 re f Q 0 0 1 rg 0 0 200 200 re f",
        "1 g 0 0 100 200 re f",
    );
    assert_eq!(rgb(&doc, 150, 100), BLUE);
    assert_eq!(rgb(&doc, 50, 100), BLUE);
}

#[test]
fn smask_none_ends_the_soft_mask() {
    let doc = soft_mask_page(
        "/GS1 gs 1 0 0 rg 0 0 200 200 re f /GS0 gs 0 0 1 rg 0 0 200 200 re f",
        "1 g 0 0 100 200 re f",
    );
    assert_eq!(rgb(&doc, 150, 100), BLUE);
}

#[test]
fn each_draw_under_one_soft_mask_is_masked_once() {
    let doc = soft_mask_page(
        "q /GS1 gs 0 0 1 rg 0 0 100 200 re f 100 0 100 200 re f Q",
        "0.5 g 0 0 200 200 re f",
    );
    let (left, right) = (rgb(&doc, 50, 100), rgb(&doc, 150, 100));
    assert_eq!(left, right);
    assert!(left[0] > 100 && left[0] < 160, "half-masked blue: {left:?}");
}

#[test]
fn a_masked_draw_leaves_the_clip_unchanged() {
    // Inside the clip [0 0 100 200], a draw under a uniform 0.5 mask, then,
    // after its Q, an unmasked fill: full blue inside the clip, white outside.
    let doc = soft_mask_page(
        "q 0 0 100 200 re W n q /GS1 gs 1 0 0 rg 0 0 200 200 re f Q \
         0 0 1 rg 0 0 200 200 re f Q",
        "0.5 g 0 0 200 200 re f",
    );
    assert_eq!(rgb(&doc, 50, 100), BLUE);
    assert_eq!(rgb(&doc, 150, 100), WHITE);
}

#[test]
fn a_tiling_pattern_fill_is_clipped() {
    let doc = page(
        "0 0 100 200 re W n /Pattern cs /P0 scn 0 0 200 200 re f",
        "<< /Pattern << /P0 5 0 R >> >>",
        &[(
            "/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 \
             /BBox [0 0 10 10] /XStep 10 /YStep 10 /Resources << >>",
            "0 0 1 rg 0 0 10 10 re f",
        )],
    );
    assert_eq!(rgb(&doc, 50, 100), BLUE);
    assert_eq!(rgb(&doc, 150, 100), WHITE);
}

#[test]
fn shading_with_sh_is_clipped_and_q_restores_it() {
    let doc = page(
        "q 0 0 100 200 re W n /Sh0 sh Q 0 0 1 rg 150 0 50 200 re f",
        "<< /Shading << /Sh0 << /ShadingType 2 /ColorSpace /DeviceRGB \
         /Coords [0 0 200 0] /Function << /FunctionType 2 /Domain [0 1] \
         /C0 [1 0 0] /C1 [1 0 0] /N 1 >> >> >> >>",
        &[],
    );
    assert_eq!(rgb(&doc, 50, 100), [255, 0, 0]);
    assert_eq!(rgb(&doc, 120, 100), WHITE);
    assert_eq!(rgb(&doc, 175, 100), BLUE);
}
