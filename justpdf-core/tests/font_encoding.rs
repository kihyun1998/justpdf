//! Text extraction through a simple font's `/Encoding` when there is no ToUnicode.

use justpdf_core::PdfDocument;
use justpdf_core::page::get_page;
use justpdf_core::text::extract_page_text_string;

/// A one-page PDF: Helvetica as `/F1` with the given `/Encoding` value, the
/// page content `content`, and `extra` objects numbered from 6.
fn pdf(encoding: &str, content: &str, extra: &[&str]) -> PdfDocument {
    let font =
        format!("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding {encoding} >>");
    let stream = format!(
        "<< /Length {} >>\nstream\n{content}\nendstream",
        content.len()
    );
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_string(),
        font,
        stream,
    ];
    objects.extend(extra.iter().map(|s| s.to_string()));

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

/// The extracted text of `pdf(encoding, "BT /F1 12 Tf 10 100 Td {show} Tj ET")`.
fn extract(encoding: &str, show: &str, extra: &[&str]) -> String {
    let doc = pdf(
        encoding,
        &format!("BT /F1 12 Tf 10 100 Td {show} Tj ET"),
        extra,
    );
    let page = get_page(&doc, 0).unwrap();
    extract_page_text_string(&doc, &page)
        .unwrap()
        .trim()
        .to_string()
}

#[test]
fn differences_override_the_base_encoding() {
    assert_eq!(extract("<< /Differences [65 /B] >>", "(A)", &[]), "B");
}

#[test]
fn mac_roman_decodes_through_its_own_table() {
    // 0x8E is eacute in MacRoman, Zcaron in WinAnsi.
    assert_eq!(extract("/MacRomanEncoding", "<8E>", &[]), "é");
}

#[test]
fn differences_on_a_win_ansi_base_override_only_the_listed_codes() {
    let enc = "<< /BaseEncoding /WinAnsiEncoding /Differences [65 /B] >>";
    // A -> B (listed), B -> B, 0x80 -> Euro (WinAnsi), 0x8E -> Zcaron (WinAnsi)
    assert_eq!(extract(enc, "<41 42 80 8E>", &[]), "BB€Ž");
}

#[test]
fn differences_run_assigns_consecutive_codes() {
    let enc = "<< /Differences [65 /x /y 97 /z] >>";
    assert_eq!(extract(enc, "(ABa)", &[]), "xyz");
}

#[test]
fn unknown_glyph_name_yields_no_letter() {
    let out = extract("<< /Differences [65 /g123] >>", "(AC)", &[]);
    assert!(!out.contains('A'), "{out:?}");
    assert!(out.contains('C'), "{out:?}");
}

#[test]
fn uni_and_u_glyph_names_decode() {
    let enc = "<< /Differences [65 /uni00E9 /u1F600 /f_i] >>";
    assert_eq!(extract(enc, "(ABC)", &[]), "é😀fi");
}

#[test]
fn to_unicode_wins_over_differences() {
    let cmap = "/CIDInit /ProcSet findresource begin 12 dict begin begincmap \
                /CMapName /Test def 1 begincodespacerange <00> <FF> endcodespacerange \
                1 beginbfchar <41> <005A> endbfchar endcmap CMapName currentdict /CMap defineresource pop end end";
    let tounicode = format!("<< /Length {} >>\nstream\n{cmap}\nendstream", cmap.len());
    let doc = {
        let font_with_tu = "<< /Differences [65 /B] >> /ToUnicode 6 0 R";
        pdf(
            font_with_tu,
            "BT /F1 12 Tf 10 100 Td (A) Tj ET",
            &[&tounicode],
        )
    };
    let page = get_page(&doc, 0).unwrap();
    assert_eq!(extract_page_text_string(&doc, &page).unwrap().trim(), "Z");
}

#[test]
fn standard_encoding_decodes_per_annex_d() {
    // 0x27 quoteright, 0x60 quoteleft, 0xA4 fraction, 0xE1 AE in StandardEncoding.
    assert_eq!(
        extract("/StandardEncoding", "<27 60 A4 E1>", &[]),
        "\u{2019}\u{2018}\u{2044}Æ"
    );
}

#[test]
fn indirect_encoding_dictionary_is_resolved() {
    assert_eq!(
        extract("6 0 R", "(A)", &["<< /Differences [65 /B] >>"]),
        "B"
    );
}

#[test]
fn indirect_encoding_name_is_resolved() {
    assert_eq!(extract("6 0 R", "<8E>", &["/MacRomanEncoding"]), "é");
}
