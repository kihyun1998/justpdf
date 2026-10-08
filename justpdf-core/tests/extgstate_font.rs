//! Text extraction applies the font and size an ExtGState's `/Font` sets
//! through `gs`.

use justpdf_core::PdfDocument;
use justpdf_core::page::get_page;
use justpdf_core::text::{PageText, extract_page_text};

/// A one-page PDF. The page's `/Font` names Helvetica (object 4) as `/F1`
/// and Courier (object 5) as `/F2`; its `/ExtGState` maps `/GS1` to
/// `ext_gstate`. `extra` are objects numbered from 6.
fn pdf(ext_gstate: &str, content: &str, extra: &[&str]) -> PdfDocument {
    let stream = format!(
        "<< /Length {} >>\nstream\n{content}\nendstream",
        content.len()
    );
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Resources << /Font << /F1 4 0 R /F2 5 0 R >> /ExtGState << /GS1 {ext_gstate} >> >> /Contents {} 0 R >>",
            6 + extra.len()
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Courier >>".to_string(),
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

fn extract(ext_gstate: &str, content: &str, extra: &[&str]) -> PageText {
    let doc = pdf(ext_gstate, content, extra);
    let page = get_page(&doc, 0).unwrap();
    extract_page_text(&doc, &page).unwrap()
}

/// The text and width of each word, top line first, widths rounded to 0.01.
fn words(text: &PageText) -> Vec<(String, f64)> {
    text.lines
        .iter()
        .flat_map(|l| &l.words)
        .map(|w| (w.text.clone(), (w.width * 100.0).round() / 100.0))
        .collect()
}

const COURIER_12: &str = "<< /Type /ExtGState /Font [5 0 R 12] >>";

/// Courier at 12 pt: 3 × 0.6 × 12.
const COURIER_ABC: f64 = 21.6;

#[test]
fn gs_sets_the_font_and_size_with_no_tf() {
    // 20 pt, not the default 12: 3 × 0.6 × 20.
    let text = extract(
        "<< /Type /ExtGState /Font [5 0 R 20] >>",
        "BT 10 100 Td /GS1 gs (ABC) Tj ET",
        &[],
    );
    assert_eq!(words(&text), vec![("ABC".to_string(), 36.0)]);
    assert!((text.chars[0].font_size - 20.0).abs() < 1e-9);
}

#[test]
fn gs_replaces_the_font_tf_set() {
    // Helvetica's ABC at 12 pt is 24.67 wide.
    let text = extract(
        COURIER_12,
        "BT /F1 12 Tf 10 200 Td (ABC) Tj 0 -100 Td /GS1 gs (ABC) Tj ET",
        &[],
    );
    assert_eq!(
        words(&text),
        vec![("ABC".to_string(), 24.67), ("ABC".to_string(), COURIER_ABC)]
    );
    // A font set by gs has no resource name.
    assert_eq!(text.chars[0].font_name, "F1");
    assert_eq!(text.chars[3].font_name, "");
}

#[test]
fn q_and_q_restore_the_font_from_before_gs() {
    let text = extract(
        COURIER_12,
        "BT /F1 12 Tf ET q BT /GS1 gs 10 200 Td (ABC) Tj ET Q BT 10 100 Td (ABC) Tj ET",
        &[],
    );
    assert_eq!(
        words(&text),
        vec![("ABC".to_string(), COURIER_ABC), ("ABC".to_string(), 24.67)]
    );
}

#[test]
fn a_font_named_only_by_the_ext_gstate_is_decoded_through_its_to_unicode() {
    // Object 6 is in no /Font dictionary; its ToUnicode maps A, B, C to X, Y, Z.
    let cmap = "/CIDInit /ProcSet findresource begin 12 dict begin begincmap 1 begincodespacerange <00> <FF> endcodespacerange 1 beginbfrange <41> <43> <0058> endbfrange endcmap end end";
    let to_unicode = format!("<< /Length {} >>\nstream\n{cmap}\nendstream", cmap.len());
    let text = extract(
        "<< /Font [6 0 R 10] >>",
        "BT 10 100 Td /GS1 gs (ABC) Tj ET",
        &[
            "<< /Type /Font /Subtype /Type1 /BaseFont /Mono /FirstChar 65 /LastChar 67 /Widths [500 500 500] /ToUnicode 7 0 R >>",
            &to_unicode,
        ],
    );
    assert_eq!(words(&text), vec![("XYZ".to_string(), 15.0)]);
}

#[test]
fn a_malformed_font_entry_changes_nothing() {
    for ext_gstate in [
        "<< /Font [5 0 R] >>",
        "<< /Font [5 0 R 12 13] >>",
        "<< /Font [/F2 12] >>",
        "<< /Font [5 0 R /Twelve] >>",
        "<< /Font [6 0 R 12] >>",
        "<< /Font [99 0 R 12] >>",
        "<< /Font 5 0 R >>",
    ] {
        // Object 6 is a number, not a font; object 99 does not exist.
        let text = extract(
            ext_gstate,
            "BT /F1 12 Tf 10 100 Td /GS1 gs (ABC) Tj ET",
            &["42"],
        );
        assert_eq!(
            words(&text),
            vec![("ABC".to_string(), 24.67)],
            "{ext_gstate}"
        );
        assert_eq!(text.chars[0].font_name, "F1", "{ext_gstate}");
    }
}
