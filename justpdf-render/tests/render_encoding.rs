//! Simple TrueType glyphs are chosen through the font's `/Encoding`
//! (ISO 32000-2 §9.6.6.4), not by reading the code as Unicode.

mod common;

use common::{Plain, Stream, page};
use justpdf_core::PdfDocument;
use justpdf_core::font::subset::subset_font;
use justpdf_core::ttf_parser;
use justpdf_render::{RenderOptions, render_page_to_pixmap};

const NOTO_SANS: &[u8] = include_bytes!("../../justpdf-core/tests/fixtures/NotoSans-Regular.ttf");

/// Noto Sans cut down to `.notdef`, `A`, `B`, `é` and `Ž`, as hex.
fn program_hex() -> String {
    let face = ttf_parser::Face::parse(NOTO_SANS, 0).unwrap();
    let mut gids = vec![0u16];
    gids.extend(['A', 'B', 'é', 'Ž'].map(|c| face.glyph_index(c).unwrap().0));
    let data = subset_font(NOTO_SANS, &gids).unwrap().data;
    let mut s: String = data.iter().map(|b| format!("{b:02X}")).collect();
    s.push('>');
    s
}

/// A page showing the hex string `shown` at 60 pt in a non-symbolic embedded
/// TrueType font whose `/Encoding` entry is `encoding` (object 8, when given,
/// is an extra object such as an encoding dictionary).
fn doc(encoding: &str, shown: &str, extra: Option<&str>) -> PdfDocument {
    let font = format!(
        "<< /Type /Font /Subtype /TrueType /BaseFont /NotoSans /FirstChar 0 /LastChar 255 \
         /Widths [{}] /FontDescriptor 6 0 R /Encoding {encoding} >>",
        vec!["600"; 256].join(" ")
    );
    let descriptor = "<< /Type /FontDescriptor /FontName /NotoSans /Flags 32 \
                      /FontBBox [0 0 1000 1000] /ItalicAngle 0 /Ascent 1000 /Descent 0 \
                      /CapHeight 700 /StemV 80 /FontFile2 7 0 R >>";
    let program = program_hex();
    let content = format!("BT /F1 60 Tf 10 20 Td <{shown}> Tj ET");
    let mut rest = vec![
        Plain(&font),
        Plain(descriptor),
        Stream("/Filter /ASCIIHexDecode", &program),
    ];
    if let Some(obj) = extra {
        rest.push(Plain(obj));
    }
    page("<< /Font << /F1 5 0 R >> >>", &content, rest)
}

fn pixels(doc: &PdfDocument) -> Vec<u8> {
    render_page_to_pixmap(doc, 0, &RenderOptions::default())
        .unwrap()
        .data
}

#[test]
fn differences_choose_the_named_glyph() {
    let differences = pixels(&doc(
        "<< /BaseEncoding /WinAnsiEncoding /Differences [65 /B] >>",
        "41",
        None,
    ));
    let b = pixels(&doc("/WinAnsiEncoding", "42", None));
    let a = pixels(&doc("/WinAnsiEncoding", "41", None));
    assert_ne!(
        a, b,
        "A and B must draw differently for this test to mean anything"
    );
    assert_eq!(differences, b);
}

#[test]
fn indirect_encoding_dictionary_is_used() {
    let differences = pixels(&doc(
        "8 0 R",
        "41",
        Some("<< /BaseEncoding /WinAnsiEncoding /Differences [65 /B] >>"),
    ));
    let b = pixels(&doc("/WinAnsiEncoding", "42", None));
    assert_eq!(differences, b);
}

#[test]
fn mac_roman_code_draws_its_mac_roman_glyph() {
    // 0x8E is eacute in MacRoman and Zcaron in WinAnsi.
    let mac = pixels(&doc("/MacRomanEncoding", "8E", None));
    let eacute = pixels(&doc("/WinAnsiEncoding", "E9", None));
    let zcaron = pixels(&doc("/WinAnsiEncoding", "8E", None));
    assert_ne!(eacute, zcaron);
    assert_eq!(mac, eacute);
}

#[test]
fn a_code_with_no_candidate_draws_nothing() {
    // 0x90 has no WinAnsi glyph, no cmap entry and is past the glyph count: no box is drawn.
    let unmapped = pixels(&doc("<< /Differences [144 /g123] >>", "90", None));
    let blank = pixels(&doc("/WinAnsiEncoding", "", None));
    assert_eq!(unmapped, blank);
}

#[test]
fn a_glyph_without_an_outline_draws_nothing() {
    // The space glyph has no contours; it used to be drawn as a placeholder box.
    let space = pixels(&doc("/WinAnsiEncoding", "20", None));
    let empty = pixels(&doc("/WinAnsiEncoding", "", None));
    assert_eq!(space, empty);
}
