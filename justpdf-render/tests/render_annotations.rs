//! Annotation appearances (#206): the `/AP /N` stream (or the `/AS` entry of a
//! state dictionary) is decoded, mapped from its `/BBox` onto the
//! annotation's `/Rect` as ISO 32000-2 §12.5.5 gives, and clipped to `/BBox`.

use justpdf_core::PdfDocument;
use justpdf_core::writer::encode_flate;
use justpdf_render::{RenderOptions, render_page_to_pixmap};

/// The annotation rectangle every test uses, on a 400 × 600 page.
const RECT: &str = "[100 500 300 520]";

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

/// A 400 × 600 page with an empty content stream and one annotation (object
/// 5) whose dictionary entries are `annot`; appearance streams follow as
/// objects 6, 7, ….
fn page_with(annot: &str, streams: Vec<(String, Vec<u8>)>) -> PdfDocument {
    let mut objects = vec![
        ("<< /Type /Catalog /Pages 2 0 R >>".to_string(), None),
        (
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            None,
        ),
        (
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 600] /Resources << >> \
             /Contents 4 0 R /Annots [5 0 R] >>"
                .to_string(),
            None,
        ),
        (String::new(), Some(Vec::new())),
        (format!("<< /Type /Annot {annot} >>"), None),
    ];
    for (dict, data) in streams {
        objects.push((dict, Some(data)));
    }
    pdf(objects)
}

/// A Form XObject dictionary with `/BBox` `bbox` and the `extra` entries.
fn form(bbox: &str, extra: &str) -> String {
    format!("/Type /XObject /Subtype /Form /BBox {bbox} {extra}")
}

/// Pixels painted `colour` (red or blue): (inside RECT, outside RECT).
fn painted(doc: &PdfDocument, blue: bool) -> (usize, usize) {
    let px = render_page_to_pixmap(doc, 0, &RenderOptions::default()).unwrap();
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
            // RECT in device pixels: x 100..300, y (from the top) 80..100.
            if (100..300).contains(&x) && (80..100).contains(&y) {
                inside += 1;
            } else {
                outside += 1;
            }
        }
    }
    (inside, outside)
}

const FULL: usize = 200 * 20;

fn red_square(bbox: &str, extra: &str, content: &str) -> PdfDocument {
    page_with(
        &format!("/Subtype /Square /Rect {RECT} /AP << /N 6 0 R >>"),
        vec![(form(bbox, extra), content.as_bytes().to_vec())],
    )
}

#[test]
fn a_flate_compressed_appearance_is_drawn() {
    let data = encode_flate(b"1 0 0 rg 0 0 200 20 re f").unwrap();
    let doc = page_with(
        &format!("/Subtype /Square /Rect {RECT} /AP << /N 6 0 R >>"),
        vec![(form("[0 0 200 20]", "/Filter /FlateDecode"), data)],
    );
    assert_eq!(painted(&doc, false), (FULL, 0));
}

#[test]
fn a_local_bbox_is_mapped_onto_the_rect() {
    let doc = red_square("[0 0 200 20]", "", "1 0 0 rg 0 0 200 20 re f");
    assert_eq!(painted(&doc, false), (FULL, 0));
}

#[test]
fn an_absolute_bbox_lands_on_the_same_pixels() {
    let doc = red_square("[100 500 300 520]", "", "1 0 0 rg 100 500 200 20 re f");
    assert_eq!(painted(&doc, false), (FULL, 0));
}

#[test]
fn a_unit_bbox_is_scaled_to_fill_the_rect() {
    let doc = red_square("[0 0 1 1]", "", "1 0 0 rg 0 0 1 1 re f");
    assert_eq!(painted(&doc, false), (FULL, 0));
}

#[test]
fn a_form_matrix_is_applied_before_the_mapping() {
    let doc = red_square(
        "[0 0 200 20]",
        "/Matrix [1 0 0 1 -100 -500]",
        "1 0 0 rg 0 0 200 20 re f",
    );
    assert_eq!(painted(&doc, false), (FULL, 0));
}

#[test]
fn content_outside_the_bbox_is_clipped() {
    let doc = red_square("[0 0 200 20]", "", "1 0 0 rg -50 -50 400 400 re f");
    assert_eq!(painted(&doc, false), (FULL, 0));
}

#[test]
fn a_state_dictionary_draws_the_state_as_names() {
    let streams = || {
        vec![
            (
                form("[0 0 200 20]", ""),
                b"1 0 0 rg 0 0 200 20 re f".to_vec(),
            ),
            (
                form("[0 0 200 20]", ""),
                b"0 0 1 rg 0 0 200 20 re f".to_vec(),
            ),
        ]
    };
    let with = |state: &str| {
        page_with(
            &format!(
                "/Subtype /Widget /Rect {RECT} {state} /AP << /N << /On 6 0 R /Off 7 0 R >> >>"
            ),
            streams(),
        )
    };
    let on = with("/AS /On");
    assert_eq!(
        (painted(&on, false), painted(&on, true)),
        ((FULL, 0), (0, 0))
    );
    let off = with("/AS /Off");
    assert_eq!(
        (painted(&off, false), painted(&off, true)),
        ((0, 0), (FULL, 0))
    );
    let none = with("");
    assert_eq!(
        (painted(&none, false), painted(&none, true)),
        ((0, 0), (0, 0))
    );
}

#[test]
fn a_transparency_group_appearance_is_mapped_and_clipped_too() {
    let doc = red_square(
        "[0 0 200 20]",
        "/Group << /S /Transparency >>",
        "1 0 0 rg -50 -50 400 400 re f",
    );
    assert_eq!(painted(&doc, false), (FULL, 0));
}

#[test]
fn hidden_and_no_view_annotations_draw_nothing() {
    for flags in [2, 32] {
        let doc = page_with(
            &format!("/Subtype /Square /F {flags} /Rect {RECT} /AP << /N 6 0 R >>"),
            vec![(
                form("[0 0 200 20]", ""),
                b"1 0 0 rg 0 0 200 20 re f".to_vec(),
            )],
        );
        assert_eq!(painted(&doc, false), (0, 0), "/F {flags}");
    }
}

#[test]
fn a_degenerate_bbox_draws_nothing() {
    let doc = red_square("[0 0 0 20]", "", "1 0 0 rg 0 0 200 20 re f");
    assert_eq!(painted(&doc, false), (0, 0));
}

#[test]
fn one_annotations_clip_does_not_cut_the_next() {
    // Two annotations: the first clips to RECT; the second, blue, sits
    // below it at [100 100 300 120] (device y 480..500).
    let mut objects = vec![
        ("<< /Type /Catalog /Pages 2 0 R >>".to_string(), None),
        (
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            None,
        ),
        (
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 600] /Resources << >> \
             /Contents 4 0 R /Annots [5 0 R 7 0 R] >>"
                .to_string(),
            None,
        ),
        (String::new(), Some(Vec::new())),
        (
            format!("<< /Type /Annot /Subtype /Square /Rect {RECT} /AP << /N 6 0 R >> >>"),
            None,
        ),
        (format!("<< {} >>", form("[0 0 200 20]", "")), None),
        (
            "<< /Type /Annot /Subtype /Square /Rect [100 100 300 120] /AP << /N 8 0 R >> >>"
                .to_string(),
            None,
        ),
    ];
    objects[5] = (
        form("[0 0 200 20]", ""),
        Some(b"1 0 0 rg 0 0 200 20 re f".to_vec()),
    );
    objects.push((
        form("[0 0 200 20]", ""),
        Some(b"0 0 1 rg 0 0 200 20 re f".to_vec()),
    ));
    let doc = pdf(objects);
    let px = render_page_to_pixmap(&doc, 0, &RenderOptions::default()).unwrap();
    let blue = (480..500)
        .flat_map(|y| (100..300).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let i = ((y * px.width + x) * 4) as usize;
            px.data[i + 2] > 200 && px.data[i] < 60
        })
        .count();
    assert_eq!(blue, FULL);
}
