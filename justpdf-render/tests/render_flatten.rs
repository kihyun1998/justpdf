//! Flattening form widgets into page content (#40): each normal appearance is
//! drawn through ISO 32000-2 §12.5.5 onto its widget's `/Rect` and named in
//! the page's `/Resources`, and every widget is removed.

use justpdf_core::content::{Operand, parse_content_stream};
use justpdf_core::form::{flatten_form, parse_acroform, set_field_value};
use justpdf_core::writer::DocumentModifier;
use justpdf_core::{PdfDict, PdfDocument, PdfObject};
use justpdf_render::{RenderOptions, render_page_to_pixmap};

/// The widget rectangle the tests use, on a 400 × 600 page.
const RECT: &str = "[100 500 300 520]";

/// A PDF whose object `n` (1-based) is `objects[n - 1]`: a dictionary and an
/// optional stream body.
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

/// A 400 × 600 page whose content is `content`, with resources `resources`,
/// and annotations: object 5 is the widget with entries `widget`; the
/// AcroForm lists it. Appearance streams follow as objects 6, 7, ….
fn form_page(
    content: &str,
    resources: &str,
    widget: &str,
    streams: Vec<(String, Vec<u8>)>,
) -> PdfDocument {
    let mut objects = vec![
        (
            "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [5 0 R] >> >>".to_string(),
            None,
        ),
        (
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            None,
        ),
        (
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 600] /Resources {resources} \
                 /Contents 4 0 R /Annots [5 0 R] >>"
            ),
            None,
        ),
        (String::new(), Some(content.as_bytes().to_vec())),
        (
            format!(
                "<< /Type /Annot /Subtype /Widget /T (f) {} {widget} >>",
                if widget.contains("/FT") {
                    ""
                } else {
                    "/FT /Tx"
                }
            ),
            None,
        ),
    ];
    for (dict, data) in streams {
        objects.push((dict, Some(data)));
    }
    pdf(objects)
}

fn form(bbox: &str, extra: &str) -> String {
    format!("/Type /XObject /Subtype /Form /BBox {bbox} {extra}")
}

fn red(bbox: &str, extra: &str, content: &str) -> (String, Vec<u8>) {
    (
        form(bbox, extra),
        format!("1 0 0 rg {content}").into_bytes(),
    )
}

/// `doc` flattened and reopened; no widget is left, so whatever is drawn
/// comes from the page content.
fn flattened(doc: &PdfDocument) -> PdfDocument {
    let mut m = DocumentModifier::from_document(doc).unwrap();
    flatten_form(&mut m, doc).unwrap();
    let out = PdfDocument::from_bytes(m.build().unwrap()).unwrap();
    assert_eq!(widgets_left(&out), 0, "a widget is left after flattening");
    out
}

/// Pixels painted red, or blue: (inside RECT, outside RECT).
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
            if hit {
                if (100..300).contains(&x) && (80..100).contains(&y) {
                    inside += 1;
                } else {
                    outside += 1;
                }
            }
        }
    }
    (inside, outside)
}

const FULL: usize = 200 * 20;

fn page_dict(doc: &PdfDocument) -> PdfDict {
    let page = justpdf_core::page::collect_pages(doc).unwrap()[0]
        .page_ref
        .clone();
    match doc.resolve(&page).unwrap() {
        PdfObject::Dict(d) => d,
        other => panic!("page {other:?}"),
    }
}

/// The page's decoded content streams, concatenated.
fn page_content(doc: &PdfDocument) -> Vec<u8> {
    let refs = match page_dict(doc).get(b"Contents").cloned() {
        Some(PdfObject::Array(a)) => a,
        Some(other) => vec![other],
        None => Vec::new(),
    };
    let mut all = Vec::new();
    for r in refs {
        let PdfObject::Reference(r) = r else { continue };
        if let PdfObject::Stream { dict, data } = doc.resolve(&r).unwrap() {
            all.extend(doc.decode_stream(&dict, &data).unwrap());
            all.push(b'\n');
        }
    }
    all
}

fn widgets_left(doc: &PdfDocument) -> usize {
    let annots = match page_dict(doc).get(b"Annots").cloned() {
        Some(PdfObject::Array(a)) => a,
        _ => return 0,
    };
    annots
        .iter()
        .filter(|a| {
            let d = match a {
                PdfObject::Reference(r) => doc.resolve(r).unwrap(),
                other => (*other).clone(),
            };
            d.as_dict().and_then(|d| d.get_name(b"Subtype")) == Some(b"Widget")
        })
        .count()
}

#[test]
fn every_xobject_name_the_flattened_content_uses_is_in_the_resources() {
    let doc = form_page(
        "",
        "<< /XObject << /Flat0 << >> >> >>",
        &format!("/Rect {RECT} /AP << /N 6 0 R >>"),
        vec![red("[0 0 200 20]", "", "0 0 200 20 re f")],
    );
    let out = flattened(&doc);
    let names: Vec<Vec<u8>> = parse_content_stream(&page_content(&out))
        .unwrap()
        .into_iter()
        .filter(|op| op.operator == b"Do")
        .filter_map(|op| match op.operands.first() {
            Some(Operand::Name(n)) => Some(n.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(names.len(), 1);
    let resources = page_dict(&out).get_dict(b"Resources").cloned().unwrap();
    let xobjects = resources.get_dict(b"XObject").cloned().unwrap();
    for name in &names {
        assert!(
            xobjects
                .get(name)
                .is_some_and(|o| matches!(o, PdfObject::Reference(_))),
            "{name:?}"
        );
    }
    // The name that was already there is kept and not reused.
    assert!(xobjects.get(b"Flat0").is_some());
    assert_ne!(names[0], b"Flat0");
    assert_eq!(painted(&out, false), (FULL, 0));
}

#[test]
fn a_bbox_the_size_of_the_rect_is_only_translated() {
    let doc = form_page(
        "",
        "<< >>",
        &format!("/Rect {RECT} /AP << /N 6 0 R >>"),
        vec![red("[0 0 200 20]", "", "0 0 200 20 re f")],
    );
    let out = flattened(&doc);
    let cm: Vec<f64> = parse_content_stream(&page_content(&out))
        .unwrap()
        .into_iter()
        .find(|op| op.operator == b"cm")
        .unwrap()
        .operands
        .iter()
        .map(|o| match o {
            Operand::Integer(i) => *i as f64,
            Operand::Real(r) => *r,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(cm, [1.0, 0.0, 0.0, 1.0, 100.0, 500.0]);
}

#[test]
fn a_scaled_or_matrixed_appearance_lands_on_the_rect() {
    for (bbox, extra, content) in [
        ("[0 0 1 1]", "", "0 0 1 1 re f"),
        (
            "[0 0 200 20]",
            "/Matrix [1 0 0 1 -100 -500]",
            "0 0 200 20 re f",
        ),
        ("[100 500 300 520]", "", "100 500 200 20 re f"),
    ] {
        let doc = form_page(
            "",
            "<< >>",
            &format!("/Rect {RECT} /AP << /N 6 0 R >>"),
            vec![red(bbox, extra, content)],
        );
        assert_eq!(
            painted(&flattened(&doc), false),
            (FULL, 0),
            "{bbox} {extra}"
        );
    }
}

#[test]
fn a_checkbox_is_drawn_in_the_state_its_as_names() {
    for (state, expect_red) in [("/On", true), ("/Off", false)] {
        let doc = form_page(
            "",
            "<< >>",
            &format!("/FT /Btn /Rect {RECT} /AS {state} /AP << /N << /On 6 0 R /Off 7 0 R >> >>"),
            vec![
                red("[0 0 200 20]", "", "0 0 200 20 re f"),
                (
                    form("[0 0 200 20]", ""),
                    b"0 0 1 rg 0 0 200 20 re f".to_vec(),
                ),
            ],
        );
        let out = flattened(&doc);
        let (r, b) = (painted(&out, false), painted(&out, true));
        if expect_red {
            assert_eq!((r, b), ((FULL, 0), (0, 0)), "{state}");
        } else {
            assert_eq!((r, b), ((0, 0), (FULL, 0)), "{state}");
        }
    }
}

#[test]
fn widgets_without_an_appearance_are_removed_too() {
    let doc = form_page("", "<< >>", &format!("/Rect {RECT}"), Vec::new());
    assert_eq!(widgets_left(&flattened(&doc)), 0);
}

#[test]
fn a_hidden_widget_is_removed_and_not_drawn() {
    for flags in [2, 32] {
        let doc = form_page(
            "",
            "<< >>",
            &format!("/F {flags} /Rect {RECT} /AP << /N 6 0 R >>"),
            vec![red("[0 0 200 20]", "", "0 0 200 20 re f")],
        );
        let out = flattened(&doc);
        assert_eq!(widgets_left(&out), 0, "/F {flags}");
        assert_eq!(painted(&out, false), (0, 0), "/F {flags}");
    }
}

#[test]
fn an_unbalanced_cm_in_the_page_content_does_not_move_the_appearance() {
    let doc = form_page(
        "2 0 0 2 0 0 cm",
        "<< >>",
        &format!("/Rect {RECT} /AP << /N 6 0 R >>"),
        vec![red("[0 0 200 20]", "", "0 0 200 20 re f")],
    );
    assert_eq!(painted(&flattened(&doc), false), (FULL, 0));
}

#[test]
fn a_value_filled_in_the_same_modifier_is_the_one_flattened() {
    let helv = "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>";
    let mut objects = vec![
        (
            format!(
                "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [5 0 R] \
                 /DR << /Font << /Helv {helv} >> >> >> >>"
            ),
            None,
        ),
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
        (
            format!(
                "<< /Type /Annot /Subtype /Widget /FT /Tx /T (f) /V (old) \
                 /DA (/Helv 12 Tf 0 g) /MK << /BG [1 0 0] >> /Rect {RECT} /AP << /N 6 0 R >> >>"
            ),
            None,
        ),
    ];
    // The old appearance is blue; the one filling writes has the red /MK background.
    objects.push((
        form("[0 0 200 20]", ""),
        Some(b"0 0 1 rg 0 0 200 20 re f".to_vec()),
    ));
    let doc = pdf(objects);
    let form = parse_acroform(&doc).unwrap().unwrap();
    let mut m = DocumentModifier::from_document(&doc).unwrap();
    set_field_value(&mut m, &form, "f", PdfObject::String(b"new".to_vec())).unwrap();
    flatten_form(&mut m, &doc).unwrap();
    let out = PdfDocument::from_bytes(m.build().unwrap()).unwrap();
    assert_eq!(widgets_left(&out), 0);
    assert_eq!(painted(&out, true), (0, 0));
    assert!(
        painted(&out, false).0 > FULL / 2,
        "{:?}",
        painted(&out, false)
    );
}
