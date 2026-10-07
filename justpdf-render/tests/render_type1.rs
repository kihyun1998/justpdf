//! Type1 font programs (`/FontFile`) are drawn from their charstrings, not as
//! placeholder boxes.

mod common;

use common::{Plain, Stream, page};
use justpdf_core::PdfDocument;
use justpdf_render::{RenderOptions, render_page_to_pixmap};

/// AMS Computer Modern Roman 10 (SIL OFL 1.1, `fixtures/OFL-AMS.txt`), as a PFB.
const CMR10_PFB: &[u8] = include_bytes!("fixtures/cmr10.pfb");

/// The PFB's segments joined: the cleartext + binary eexec form a PDF `/FontFile`
/// holds, and the three segment lengths (`/Length1`, `/Length2`, `/Length3`).
fn font_file() -> (Vec<u8>, [usize; 3]) {
    let (mut data, mut lengths, mut i) = (Vec::new(), Vec::new(), 0);
    while CMR10_PFB[i] == 0x80 && CMR10_PFB[i + 1] != 3 {
        let n = u32::from_le_bytes(CMR10_PFB[i + 2..i + 6].try_into().unwrap()) as usize;
        data.extend_from_slice(&CMR10_PFB[i + 6..i + 6 + n]);
        lengths.push(n);
        i += 6 + n;
    }
    (data, lengths.try_into().unwrap())
}

fn hex(data: &[u8]) -> String {
    let mut s: String = data.iter().map(|b| format!("{b:02X}")).collect();
    s.push('>');
    s
}

/// A page showing the hex string `shown` at `size` pt from (5, 10) in a Type1
/// font backed by `program`, whose font-dictionary entries end with `encoding`.
fn doc_with(
    program: &[u8],
    lengths: [usize; 3],
    size: u32,
    encoding: &str,
    shown: &str,
) -> PdfDocument {
    let font = format!(
        "<< /Type /Font /Subtype /Type1 /BaseFont /CMR10 /FirstChar 0 /LastChar 255 \
         /Widths [{}] /FontDescriptor 6 0 R {encoding} >>",
        vec!["750"; 256].join(" ")
    );
    let descriptor = "<< /Type /FontDescriptor /FontName /CMR10 /Flags 4 \
                      /FontBBox [-40 -250 1009 750] /ItalicAngle 0 /Ascent 683 /Descent -217 \
                      /CapHeight 683 /StemV 69 /FontFile 7 0 R >>";
    let stream_dict = format!(
        "/Length1 {} /Length2 {} /Length3 {} /Filter /ASCIIHexDecode",
        lengths[0], lengths[1], lengths[2]
    );
    let program = hex(program);
    let content = format!("BT /F1 {size} Tf 5 10 Td <{shown}> Tj ET");
    page(
        "<< /Font << /F1 5 0 R >> >>",
        &content,
        vec![
            Plain(&font),
            Plain(descriptor),
            Stream(&stream_dict, &program),
        ],
    )
}

fn doc(encoding: &str, shown: &str) -> PdfDocument {
    let (program, lengths) = font_file();
    doc_with(&program, lengths, 100, encoding, shown)
}

fn pixels(doc: &PdfDocument) -> Vec<u8> {
    render_page_to_pixmap(doc, 0, &RenderOptions::default())
        .unwrap()
        .data
}

fn dark(px: &[u8], x: usize, y: usize) -> bool {
    px[(y * 100 + x) * 4] < 128
}

/// cmr10 `H` at 100 pt from (5, 10): stems x 136–225 and 524–613, crossbar
/// y 340–371 (glyph units).
fn assert_h_shape(px: &[u8]) {
    assert!(dark(px, 23, 70), "left stem");
    assert!(dark(px, 42, 54), "crossbar");
    assert!(!dark(px, 42, 75), "lower counter");
    assert!(!dark(px, 42, 40), "upper counter");
}

#[test]
fn type1_draws_the_glyph_outline() {
    assert_h_shape(&pixels(&doc("", "48")));
}

#[test]
fn without_an_encoding_the_built_in_encoding_is_used() {
    // cmr10's own encoding puts ff at 0x0B.
    let builtin = pixels(&doc("", "0B"));
    let named = pixels(&doc("/Encoding << /Differences [65 /ff] >>", "41"));
    assert_ne!(builtin, pixels(&doc("", "")), "ff draws something");
    assert_ne!(builtin, pixels(&doc("", "41")), "ff and A differ");
    assert_eq!(builtin, named);
}

#[test]
fn differences_choose_the_named_glyph() {
    let differences = pixels(&doc("/Encoding << /Differences [72 /A] >>", "48"));
    assert_eq!(differences, pixels(&doc("", "41")));
    assert_ne!(differences, pixels(&doc("", "48")));
}

#[test]
fn a_glyph_name_the_font_lacks_draws_nothing() {
    let missing = pixels(&doc("/Encoding << /Differences [65 /nosuchglyph] >>", "41"));
    assert_eq!(missing, pixels(&doc("", "")));
}

#[test]
fn glyphs_are_scaled_by_the_font_matrix() {
    // The same program with FontMatrix 0.0005, drawn at 200 pt, matches 0.001 at 100 pt.
    let (program, lengths) = font_file();
    let text = b"/FontMatrix [0.001 0 0 0.001 0 0 ]";
    let half = b"/FontMatrix [.0005 0 0 .0005 0 0 ]";
    assert_eq!(text.len(), half.len());
    let at = program.windows(text.len()).position(|w| w == text).unwrap();
    let mut halved = program.clone();
    halved[at..at + text.len()].copy_from_slice(half);
    let half_at_200 = pixels(&doc_with(&halved, lengths, 200, "", "48"));
    assert_eq!(half_at_200, pixels(&doc("", "48")));
}

#[test]
fn glyphs_named_differently_are_cached_apart() {
    // Two glyphs in one string: the second must not reuse the first's outline.
    assert_ne!(pixels(&doc("", "4141")), pixels(&doc("", "4148")));
}
