//! The soft-mask dictionary's `/BC` (the backdrop a Luminosity mask's group
//! is composited over) and `/TR` (the transfer function applied to the mask
//! values), ISO 32000-2 Table 144 (#374).

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

/// A 200 × 200 page that fills red under `/GS << /SMask << smask >> >>`. The
/// mask group is object 5, with group colour space `cs` and content `g`;
/// `extra` follows as objects 6, 7, ….
fn masked(smask: &str, cs: &str, g: &str, extra: Vec<(&str, Option<&str>)>) -> PdfDocument {
    let mut objects = vec![
        ("<< /Type /Catalog /Pages 2 0 R >>".to_string(), None),
        (
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            None,
        ),
        (
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << \
                 /ExtGState << /GS << /SMask << /G 5 0 R {smask} >> >> >> >> \
                 /Contents 4 0 R >>"
            ),
            None,
        ),
        (
            String::new(),
            Some("/GS gs 1 0 0 rg 0 0 200 200 re f".to_string()),
        ),
        (
            format!(
                "/Type /XObject /Subtype /Form /BBox [0 0 200 200] \
                 /Group << /S /Transparency /CS {cs} >>"
            ),
            Some(g.to_string()),
        ),
    ];
    for (dict, body) in extra {
        objects.push((dict.to_string(), body.map(str::to_string)));
    }
    pdf(objects)
}

/// The RGB of the pixel at page point (`x`, `y`), a pixel per point.
fn rgb(doc: &PdfDocument, x: u32, y: u32) -> [u8; 3] {
    let px = render_page_to_pixmap(doc, 0, &RenderOptions::default()).unwrap();
    let i = (((px.height - 1 - y) * px.width + x) * 4) as usize;
    [px.data[i], px.data[i + 1], px.data[i + 2]]
}

const RED: [u8; 3] = [255, 0, 0];
const WHITE: [u8; 3] = [255, 255, 255];

/// A Type 2 function inverting its input: `f(x) = 1 - x`.
const INVERT: &str = "<< /FunctionType 2 /Domain [0 1] /C0 [1] /C1 [0] /N 1 >>";

// ---- /BC -------------------------------------------------------------------

#[test]
fn without_bc_the_backdrop_is_black() {
    let doc = masked("/S /Luminosity", "/DeviceGray", "", vec![]);
    assert_eq!(rgb(&doc, 100, 100), WHITE);
}

#[test]
fn a_white_bc_lets_paint_through_where_the_group_paints_nothing() {
    let doc = masked("/S /Luminosity /BC [1]", "/DeviceGray", "", vec![]);
    assert_eq!(rgb(&doc, 100, 100), RED);
}

#[test]
fn bc_shows_only_where_the_group_paints_nothing() {
    // The group paints its left half black; its right half shows /BC.
    let doc = masked(
        "/S /Luminosity /BC [1]",
        "/DeviceGray",
        "0 g 0 0 100 200 re f",
        vec![],
    );
    assert_eq!(rgb(&doc, 25, 100), WHITE);
    assert_eq!(rgb(&doc, 175, 100), RED);
}

#[test]
fn bc_is_read_in_the_group_colour_space() {
    let rgb_space = masked("/S /Luminosity /BC [1 1 1]", "/DeviceRGB", "", vec![]);
    assert_eq!(rgb(&rgb_space, 100, 100), RED);
    // CMYK white is all zeros.
    let cmyk = masked("/S /Luminosity /BC [0 0 0 0]", "/DeviceCMYK", "", vec![]);
    assert_eq!(rgb(&cmyk, 100, 100), RED);
}

#[test]
fn bc_in_an_icc_based_group_follows_its_component_count() {
    let doc = masked(
        "/S /Luminosity /BC [1]",
        "[/ICCBased 6 0 R]",
        "",
        vec![("/N 1", Some(""))],
    );
    assert_eq!(rgb(&doc, 100, 100), RED);
}

#[test]
fn an_alpha_mask_ignores_bc() {
    // The group paints nothing, so its alpha is 0 whatever /BC says.
    let doc = masked("/S /Alpha /BC [1]", "/DeviceGray", "", vec![]);
    assert_eq!(rgb(&doc, 100, 100), WHITE);
}

// ---- /TR -------------------------------------------------------------------

#[test]
fn tr_maps_the_mask_values() {
    let tr = format!("/S /Luminosity /TR {INVERT}");
    let doc = masked(&tr, "/DeviceGray", "1 g 0 0 200 200 re f", vec![]);
    assert_eq!(rgb(&doc, 100, 100), WHITE);
}

#[test]
fn tr_identity_is_no_function() {
    let doc = masked(
        "/S /Luminosity /TR /Identity",
        "/DeviceGray",
        "1 g 0 0 200 200 re f",
        vec![],
    );
    assert_eq!(rgb(&doc, 100, 100), RED);
}

#[test]
fn tr_applies_to_an_alpha_mask() {
    // An opaque group gives alpha 1; inverted, the mask is 0.
    let tr = format!("/S /Alpha /TR {INVERT}");
    let doc = masked(&tr, "/DeviceGray", "0 g 0 0 200 200 re f", vec![]);
    assert_eq!(rgb(&doc, 100, 100), WHITE);
}

#[test]
fn tr_may_be_an_indirect_postscript_function() {
    let doc = masked(
        "/S /Luminosity /TR 6 0 R",
        "/DeviceGray",
        "1 g 0 0 200 200 re f",
        vec![(
            "/FunctionType 4 /Domain [0 1] /Range [0 1]",
            Some("{ 1 exch sub }"),
        )],
    );
    assert_eq!(rgb(&doc, 100, 100), WHITE);
}

#[test]
fn a_filtered_tr_stream_is_decoded() {
    // A filtered Type 4 stream is decoded through the document (#47): the
    // inverting function maps the white group to mask 0.
    let doc = masked(
        "/S /Luminosity /TR 6 0 R",
        "/DeviceGray",
        "1 g 0 0 200 200 re f",
        vec![(
            "/FunctionType 4 /Domain [0 1] /Range [0 1] /Filter /ASCIIHexDecode",
            Some("7B2031206578636820737562207D>"),
        )],
    );
    assert_eq!(rgb(&doc, 100, 100), WHITE);
}
