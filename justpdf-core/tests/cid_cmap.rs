//! Text extraction reads a Type0 font's codes through its embedded encoding
//! CMap: split by the codespace, widths looked up by CID, ToUnicode by code.

use justpdf_core::PdfDocument;
use justpdf_core::page::get_page;
use justpdf_core::text::{TextChar, extract_page_text};

/// A one-page PDF showing the hex string `shown` at 10 pt from x = 10 in a
/// Type0 font (object 4) whose `/Encoding` is the CMap stream `cmap` (object
/// 6) and whose descendant (object 7) ends with `cid_entries`. `to_unicode`,
/// when given, is the font's ToUnicode CMap (object 8).
fn pdf(cmap: &str, cid_entries: &str, shown: &str, to_unicode: Option<&str>) -> PdfDocument {
    let stream = |dict: &str, body: &str| {
        format!(
            "<< {dict} /Length {} >>\nstream\n{body}\nendstream",
            body.len()
        )
    };
    let tu = if to_unicode.is_some() {
        "/ToUnicode 8 0 R"
    } else {
        ""
    };
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_string(),
        format!(
            "<< /Type /Font /Subtype /Type0 /BaseFont /CIDTest /Encoding 6 0 R \
             /DescendantFonts [7 0 R] {tu} >>"
        ),
        stream("", &format!("BT /F1 10 Tf 10 100 Td <{shown}> Tj ET")),
        stream("/Type /CMap /CMapName /Test", cmap),
        format!(
            "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /CIDTest \
             /CIDSystemInfo << /Registry (Adobe) /Ordering (Test) /Supplement 0 >> {cid_entries} >>"
        ),
    ];
    if let Some(body) = to_unicode {
        objects.push(stream("", body));
    }

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

fn chars(doc: &PdfDocument) -> Vec<TextChar> {
    let page = get_page(doc, 0).unwrap();
    extract_page_text(doc, &page).unwrap().chars
}

/// Each character's text, x and width, rounded to 0.01 pt.
fn placed(doc: &PdfDocument) -> Vec<(String, f64, f64)> {
    let round = |v: f64| (v * 100.0).round() / 100.0;
    chars(doc)
        .into_iter()
        .map(|c| (c.unicode, round(c.x), round(c.width)))
        .collect()
}

const CODE_41_TO_CID_300: &str = "/CIDInit /ProcSet findresource begin 12 dict begin begincmap \
     1 begincodespacerange <0000> <FFFF> endcodespacerange \
     1 begincidchar <0041> 300 endcidchar \
     endcmap CMapName currentdict /CMap defineresource pop end end";

#[test]
fn a_code_advances_by_the_width_of_the_cid_the_cmap_maps_it_to() {
    // <0041> is CID 300, 2000 units: 20 pt at 10 pt. The second <0041>
    // starts 20 pt after the first.
    let doc = pdf(
        CODE_41_TO_CID_300,
        "/DW 1000 /W [300 [2000] 65 [500]]",
        "00410041",
        None,
    );
    let xs: Vec<(f64, f64)> = placed(&doc).into_iter().map(|c| (c.1, c.2)).collect();
    assert_eq!(xs, vec![(10.0, 20.0), (30.0, 20.0)]);
}

const MIXED: &str = "/CIDInit /ProcSet findresource begin 12 dict begin begincmap \
     2 begincodespacerange <00> <7F> <8140> <9FFC> endcodespacerange \
     2 begincidrange <20> <7E> 1 <8140> <817E> 633 endcidrange \
     endcmap CMapName currentdict /CMap defineresource pop end end";

/// ToUnicode by code: <41> A, <42> B, <8140> 全.
const MIXED_TO_UNICODE: &str = "/CIDInit /ProcSet findresource begin 12 dict begin begincmap \
     2 begincodespacerange <00> <7F> <8140> <9FFC> endcodespacerange \
     3 beginbfchar <41> <0041> <42> <0042> <8140> <5168> endbfchar \
     endcmap CMapName currentdict /CMap defineresource pop end end";

#[test]
fn mixed_one_and_two_byte_codes_split_by_the_codespace() {
    // A (CID 34, 500 units), <8140> (CID 633, 2000 units), B (CID 35, 1000 by /DW).
    let doc = pdf(
        MIXED,
        "/DW 1000 /W [34 [500] 633 [2000]]",
        "41814042",
        Some(MIXED_TO_UNICODE),
    );
    assert_eq!(
        placed(&doc),
        vec![
            ("A".to_string(), 10.0, 5.0),
            ("全".to_string(), 15.0, 20.0),
            ("B".to_string(), 35.0, 10.0),
        ]
    );
}

#[test]
fn identity_h_is_unchanged() {
    let doc = pdf(
        "/Identity-H usecmap",
        "/DW 1000 /W [65 [500]]",
        "00410041",
        None,
    );
    let xs: Vec<f64> = placed(&doc).into_iter().map(|c| c.1).collect();
    assert_eq!(xs, vec![10.0, 15.0]);
}
