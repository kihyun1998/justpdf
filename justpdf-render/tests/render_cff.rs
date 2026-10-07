//! Bare CFF font programs (`/FontFile3 /Subtype /Type1C` and `/CIDFontType0C`)
//! are drawn from their charstrings, not as placeholder boxes.

mod common;

use common::{Plain, Stream, page};
use justpdf_core::PdfDocument;
use justpdf_render::{RenderOptions, render_page_to_pixmap};

/// URW NimbusSans Regular, a name-keyed bare CFF (SIL OFL 1.1, `fixtures/OFL-URW.txt`).
const NIMBUS_SANS: &[u8] = include_bytes!("fixtures/NimbusSans-Regular.cff");
/// A CID-keyed bare CFF made from it by `scripts/make-cid-cff-fixture.py`:
/// CID 300 is `H`, CID 301 is `E`.
const CID_TEST: &[u8] = include_bytes!("fixtures/cid-test.cff");
/// The same with a Top DICT `FontMatrix` of 0.0005: glyphs come out half size.
const CID_TEST_HALF: &[u8] = include_bytes!("fixtures/cid-test-half.cff");

fn hex(data: &[u8]) -> String {
    let mut s: String = data.iter().map(|b| format!("{b:02X}")).collect();
    s.push('>');
    s
}

/// A page showing the hex string `shown` at 100 pt from (5, 10) in a Type1C
/// font whose font-dictionary entries end with `encoding` (e.g. `/Encoding …`).
fn type1c(encoding: &str, shown: &str) -> PdfDocument {
    let font = format!(
        "<< /Type /Font /Subtype /Type1 /BaseFont /NimbusSans-Regular /FirstChar 0 /LastChar 255 \
         /Widths [{}] /FontDescriptor 6 0 R {encoding} >>",
        vec!["722"; 256].join(" ")
    );
    let descriptor = "<< /Type /FontDescriptor /FontName /NimbusSans-Regular /Flags 32 \
                      /FontBBox [0 0 1000 1000] /ItalicAngle 0 /Ascent 1000 /Descent 0 \
                      /CapHeight 729 /StemV 80 /FontFile3 7 0 R >>";
    let program = hex(NIMBUS_SANS);
    let content = format!("BT /F1 100 Tf 5 10 Td <{shown}> Tj ET");
    page(
        "<< /Font << /F1 5 0 R >> >>",
        &content,
        vec![
            Plain(&font),
            Plain(descriptor),
            Stream("/Subtype /Type1C /Filter /ASCIIHexDecode", &program),
        ],
    )
}

/// A page showing the two-byte CIDs `shown` at 100 pt from (5, 10) in a Type0
/// font over the CID-keyed CFF.
fn cid(shown: &str) -> PdfDocument {
    cid_font(CID_TEST, 100, shown)
}

/// A page showing `shown` at `size` pt from (5, 10) in a Type0 font over `cff`.
fn cid_font(cff: &[u8], size: u32, shown: &str) -> PdfDocument {
    let program = hex(cff);
    let content = format!("BT /F1 {size} Tf 5 10 Td <{shown}> Tj ET");
    page(
        "<< /Font << /F1 5 0 R >> >>",
        &content,
        vec![
            Plain(
                "<< /Type /Font /Subtype /Type0 /BaseFont /CIDTest /Encoding /Identity-H /DescendantFonts [6 0 R] >>",
            ),
            Plain(
                "<< /Type /Font /Subtype /CIDFontType0 /BaseFont /CIDTest \
                   /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
                   /DW 722 /FontDescriptor 7 0 R >>",
            ),
            Plain(
                "<< /Type /FontDescriptor /FontName /CIDTest /Flags 4 /FontBBox [0 0 1000 1000] \
                   /ItalicAngle 0 /Ascent 1000 /Descent 0 /CapHeight 729 /StemV 80 /FontFile3 8 0 R >>",
            ),
            Stream("/Subtype /CIDFontType0C /Filter /ASCIIHexDecode", &program),
        ],
    )
}

fn pixels(doc: &PdfDocument) -> Vec<u8> {
    render_page_to_pixmap(doc, 0, &RenderOptions::default())
        .unwrap()
        .data
}

/// Whether device pixel (x, y) of a 100x100 page at 72 dpi is dark.
fn dark(px: &[u8], x: usize, y: usize) -> bool {
    px[(y * 100 + x) * 4] < 128
}

/// `H` at 100 pt from (5, 10): stems x 83–176 and 551–644, crossbar y 332–414
/// (glyph units), so the left stem and crossbar are filled and both counters empty.
fn assert_h_shape(px: &[u8]) {
    assert!(dark(px, 18, 70), "left stem");
    assert!(dark(px, 41, 52), "crossbar");
    assert!(!dark(px, 41, 73), "lower counter");
    assert!(!dark(px, 41, 33), "upper counter");
}

#[test]
fn type1c_draws_the_glyph_outline() {
    assert_h_shape(&pixels(&type1c("/Encoding /WinAnsiEncoding", "48")));
}

#[test]
fn without_an_encoding_the_built_in_encoding_is_used() {
    assert_h_shape(&pixels(&type1c("", "48")));
}

#[test]
fn differences_choose_the_named_glyph() {
    let differences = pixels(&type1c("/Encoding << /Differences [65 /Eacute] >>", "41"));
    let eacute = pixels(&type1c("/Encoding /WinAnsiEncoding", "C9"));
    let a = pixels(&type1c("/Encoding /WinAnsiEncoding", "41"));
    assert_ne!(eacute, a);
    assert_eq!(differences, eacute);
}

#[test]
fn a_glyph_name_the_font_lacks_draws_nothing() {
    let missing = pixels(&type1c(
        "/Encoding << /Differences [65 /nosuchglyph] >>",
        "41",
    ));
    let empty = pixels(&type1c("/Encoding /WinAnsiEncoding", ""));
    assert_eq!(missing, empty);
}

#[test]
fn a_cff_space_draws_nothing() {
    let space = pixels(&type1c("/Encoding /WinAnsiEncoding", "20"));
    let empty = pixels(&type1c("/Encoding /WinAnsiEncoding", ""));
    assert_eq!(space, empty);
}

#[test]
fn cid_keyed_cff_draws_the_glyph_for_its_cid() {
    // CID 300 (0x012C) is H; GID 1 in the font, so CID != GID.
    let h = pixels(&cid("012C"));
    assert_h_shape(&h);
    assert_eq!(h, pixels(&type1c("/Encoding /WinAnsiEncoding", "48")));
    // A CID the font does not have draws nothing.
    assert_eq!(pixels(&cid("0001")), pixels(&cid("")));
}

#[test]
fn glyphs_are_scaled_by_the_font_matrix() {
    // FontMatrix 0.0005 at 200 pt is the same scale as 0.001 at 100 pt.
    let half_at_200 = pixels(&cid_font(CID_TEST_HALF, 200, "012C"));
    assert_eq!(half_at_200, pixels(&cid("012C")));
}
