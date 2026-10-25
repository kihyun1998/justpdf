//! Text extraction leaves out text in optional content hidden under the
//! default configuration, unless the caller asks for it (#219).

use justpdf_core::PdfDocument;
use justpdf_core::page::get_page;
use justpdf_core::text::{TextExtractOptions, extract_page_text, extract_page_text_with_options};

/// A one-page PDF showing `content` in Helvetica (`/F1`, object 4). The
/// catalog carries `oc_properties` (empty for none); the page's
/// `/Properties` is `properties`. `extra` are objects numbered from 5; the
/// content stream follows them.
fn pdf(oc_properties: &str, properties: &str, content: &str, extra: &[&str]) -> PdfDocument {
    let stream = format!(
        "<< /Length {} >>\nstream\n{content}\nendstream",
        content.len()
    );
    let mut objects = vec![
        format!("<< /Type /Catalog /Pages 2 0 R {oc_properties} >>"),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Resources << /Font << /F1 4 0 R >> /Properties {properties} >> /Contents {} 0 R >>",
            5 + extra.len()
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    objects.extend(extra.iter().map(|s| s.to_string()));
    objects.push(stream);

    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
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

/// OCG 5 is `L`, ON or OFF in `/D` as `state` says (`/ON` or `/OFF`).
fn oc_properties(state: &str) -> String {
    format!("/OCProperties << /OCGs [5 0 R] /D << {state} [5 0 R] >> >>")
}

const OCG: &str = "<< /Type /OCG /Name (L) >>";

/// `(Visible)` directly, `(HiddenLayer)` inside `/OC /L BDC … EMC`.
const LAYERED: &str =
    "BT /F1 12 Tf 10 200 Td (Visible) Tj 60 0 Td /OC /L BDC (HiddenLayer) Tj EMC ET";

/// The page's words in reading order: by default, and with hidden content.
fn words(doc: &PdfDocument) -> (Vec<String>, Vec<String>) {
    let page = get_page(doc, 0).unwrap();
    let collect = |text: justpdf_core::text::PageText| -> Vec<String> {
        text.lines
            .iter()
            .flat_map(|l| &l.words)
            .map(|w| w.text.clone())
            .collect()
    };
    let default = collect(extract_page_text(doc, &page).unwrap());
    let with_hidden = TextExtractOptions {
        include_hidden: true,
    };
    let all = collect(extract_page_text_with_options(doc, &page, &with_hidden).unwrap());
    (default, all)
}

fn v(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

#[test]
fn a_hidden_layer_is_left_out_unless_asked_for() {
    let doc = pdf(&oc_properties("/OFF"), "<< /L 5 0 R >>", LAYERED, &[OCG]);
    let (default, all) = words(&doc);
    assert_eq!(default, v(&["Visible"]));
    assert_eq!(all, v(&["Visible", "HiddenLayer"]));
}

#[test]
fn a_layer_that_is_on_is_extracted_either_way() {
    let doc = pdf(&oc_properties("/ON"), "<< /L 5 0 R >>", LAYERED, &[OCG]);
    let (default, all) = words(&doc);
    assert_eq!(default, v(&["Visible", "HiddenLayer"]));
    assert_eq!(all, default);
}

#[test]
fn an_ocmd_follows_its_policy_and_expression() {
    let content = "BT /F1 12 Tf 10 200 Td (Visible) Tj 60 0 Td /OC /M BDC (Member) Tj EMC ET";
    let with = |ocmd: &str| {
        let doc = pdf(
            &oc_properties("/OFF"),
            "<< /M 6 0 R >>",
            content,
            &[OCG, ocmd],
        );
        words(&doc).0
    };
    assert_eq!(
        with("<< /Type /OCMD /OCGs [5 0 R] /P /AllOff >>"),
        v(&["Visible", "Member"])
    );
    assert_eq!(
        with("<< /Type /OCMD /OCGs [5 0 R] /P /AnyOn >>"),
        v(&["Visible"])
    );
    assert_eq!(
        with("<< /Type /OCMD /VE [/Not 5 0 R] >>"),
        v(&["Visible", "Member"])
    );
}

#[test]
fn nested_marked_content_does_not_end_the_hidden_section_early() {
    let content = "BT /F1 12 Tf 10 200 Td /OC /L BDC /Span BMC (Inner) Tj EMC 60 0 Td (Outer) Tj EMC \
                   60 0 Td (After) Tj ET";
    let doc = pdf(&oc_properties("/OFF"), "<< /L 5 0 R >>", content, &[OCG]);
    assert_eq!(words(&doc).0, v(&["After"]));
}

#[test]
fn hidden_text_still_moves_the_text_position() {
    // ISO 32000-1 §8.11.3.1: the text position is updated for hidden text,
    // so (Visible) starts where it does when the hidden word is extracted.
    let content = "BT /F1 12 Tf 10 200 Td /OC /L BDC (HiddenLayer) Tj EMC (Visible) Tj ET";
    let doc = pdf(&oc_properties("/OFF"), "<< /L 5 0 R >>", content, &[OCG]);
    let page = get_page(&doc, 0).unwrap();
    let v_x = |options: &TextExtractOptions| {
        let text = extract_page_text_with_options(&doc, &page, options).unwrap();
        text.chars
            .iter()
            .find(|c| c.unicode == "V")
            .map(|c| c.x)
            .unwrap()
    };
    let hidden_out = v_x(&TextExtractOptions::default());
    let hidden_in = v_x(&TextExtractOptions {
        include_hidden: true,
    });
    // "HiddenLayer" in Helvetica at 12 pt: 5669 / 1000 × 12 = 68.028.
    assert!((hidden_out - 78.028).abs() < 1e-6, "{hidden_out}");
    assert_eq!(hidden_out, hidden_in);
}

#[test]
fn without_optional_content_everything_is_extracted() {
    let none = pdf("", "<< /L 5 0 R >>", LAYERED, &[OCG]);
    assert_eq!(words(&none).0, v(&["Visible", "HiddenLayer"]));
    let missing_name = pdf(&oc_properties("/OFF"), "<< >>", LAYERED, &[OCG]);
    assert_eq!(words(&missing_name).0, v(&["Visible", "HiddenLayer"]));
}
