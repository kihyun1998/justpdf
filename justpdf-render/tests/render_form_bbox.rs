//! A Form XObject drawn by `Do` is clipped to its `/BBox` (ISO 32000-2
//! §8.10.1), through its `/Matrix` and the CTM, and the clip in effect before
//! the `Do` is the one in effect after it (#365).

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

/// A 400 × 600 page whose content is `content`, with the forms `/F0`, `/F1`,
/// … in its resources as objects 5, 6, …. Each form is (extra dictionary
/// entries, content); every form can name every other in its own resources.
fn page_with(content: &str, forms: &[(&str, &str)]) -> PdfDocument {
    let names: String = (0..forms.len())
        .map(|i| format!("/F{i} {} 0 R ", i + 5))
        .collect();
    let mut objects = vec![
        ("<< /Type /Catalog /Pages 2 0 R >>".to_string(), None),
        (
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            None,
        ),
        (
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 600] \
                 /Resources << /XObject << {names}>> >> /Contents 4 0 R >>"
            ),
            None,
        ),
        (String::new(), Some(content.as_bytes().to_vec())),
    ];
    for (extra, body) in forms {
        objects.push((
            format!(
                "/Type /XObject /Subtype /Form {extra} \
                 /Resources << /XObject << {names}>> >>"
            ),
            Some(body.as_bytes().to_vec()),
        ));
    }
    pdf(objects)
}

/// A form content that fills the whole page red.
const FILL_RED: &str = "1 0 0 rg -1000 -1000 3000 3000 re f";

/// Pixels painted red (or blue): (inside `area`, outside it), `area` being
/// `[llx lly urx ury]` in page points (a pixel per point at the default
/// resolution).
fn painted(doc: &PdfDocument, blue: bool, area: [u32; 4]) -> (usize, usize) {
    let px = render_page_to_pixmap(doc, 0, &RenderOptions::default()).unwrap();
    let [llx, lly, urx, ury] = area;
    let (mut inside, mut outside) = (0, 0);
    for y in 0..px.height {
        for x in 0..px.width {
            let i = ((y * px.width + x) * 4) as usize;
            let [r, g, b] = [px.data[i], px.data[i + 1], px.data[i + 2]];
            let hit = if blue {
                b > 200 && r < 60 && g < 60
            } else {
                r > 200 && g < 60 && b < 60
            };
            if !hit {
                continue;
            }
            let page_y = px.height - 1 - y;
            if (llx..urx).contains(&x) && (lly..ury).contains(&page_y) {
                inside += 1;
            } else {
                outside += 1;
            }
        }
    }
    (inside, outside)
}

#[test]
fn a_form_is_clipped_to_its_bbox() {
    let doc = page_with("/F0 Do", &[("/BBox [100 100 200 200]", FILL_RED)]);
    assert_eq!(painted(&doc, false, [100, 100, 200, 200]), (100 * 100, 0));
}

#[test]
fn the_clip_follows_the_form_matrix_and_the_ctm() {
    // BBox [0 0 100 100], moved by /Matrix (+50, +50) and the CTM (+100, 0):
    // page x 150..250, y 50..150.
    let doc = page_with(
        "1 0 0 1 100 0 cm /F0 Do",
        &[("/BBox [0 0 100 100] /Matrix [1 0 0 1 50 50]", FILL_RED)],
    );
    assert_eq!(painted(&doc, false, [150, 50, 250, 150]), (100 * 100, 0));
}

#[test]
fn a_transparency_group_form_is_clipped_to_its_bbox() {
    let doc = page_with(
        "/F0 Do",
        &[(
            "/BBox [100 100 200 200] /Group << /S /Transparency >>",
            FILL_RED,
        )],
    );
    assert_eq!(painted(&doc, false, [100, 100, 200, 200]), (100 * 100, 0));
}

#[test]
fn content_after_the_do_is_not_clipped_to_the_bbox() {
    let doc = page_with(
        "/F0 Do 0 0 1 rg 300 300 50 50 re f",
        &[("/BBox [100 100 200 200]", "1 0 0 rg 100 100 10 10 re f")],
    );
    assert_eq!(painted(&doc, true, [300, 300, 350, 350]), (50 * 50, 0));
}

#[test]
fn the_page_clip_before_the_do_is_the_one_after_it() {
    // Inside the page clip [0 0 150 150] the form paints [100 100 150 150].
    // After it, a full-page blue fill is clipped to [0 0 150 150] only.
    let doc = page_with(
        "q 0 0 150 150 re W n /F0 Do 0 0 1 rg 0 0 400 600 re f Q",
        &[("/BBox [100 100 200 200]", FILL_RED)],
    );
    assert_eq!(painted(&doc, true, [0, 0, 150, 150]), (150 * 150, 0));

    let doc = page_with(
        "q 0 0 150 150 re W n /F0 Do Q",
        &[("/BBox [100 100 200 200]", FILL_RED)],
    );
    assert_eq!(painted(&doc, false, [100, 100, 150, 150]), (50 * 50, 0));
}

#[test]
fn a_nested_form_is_clipped_to_both_boxes() {
    let doc = page_with(
        "/F0 Do",
        &[
            ("/BBox [0 0 200 200]", "/F1 Do"),
            ("/BBox [100 100 400 400]", FILL_RED),
        ],
    );
    assert_eq!(painted(&doc, false, [100, 100, 200, 200]), (100 * 100, 0));
}
