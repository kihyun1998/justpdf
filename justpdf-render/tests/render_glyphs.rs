//! Glyph outlines drawn from embedded TrueType programs: each font's glyphs
//! come from its own program, whatever other fonts the page shows (#163).

mod common;

use common::{Plain, Stream, page};
use justpdf_core::PdfDocument;
use justpdf_core::font::subset::subset_font;
use justpdf_core::ttf_parser;
use justpdf_render::{RenderOptions, render_page_to_pixmap};

const NOTO_SANS: &[u8] = include_bytes!("../../justpdf-core/tests/fixtures/NotoSans-Regular.ttf");

fn be16(data: &[u8], at: usize) -> usize {
    u16::from_be_bytes([data[at], data[at + 1]]) as usize
}

fn be32(data: &[u8], at: usize) -> usize {
    u32::from_be_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]) as usize
}

/// Offset of table `tag` in a TrueType program.
fn table(data: &[u8], tag: &[u8; 4]) -> usize {
    (0..be16(data, 4))
        .map(|i| 12 + 16 * i)
        .find(|&rec| &data[rec..rec + 4] == tag)
        .map(|rec| be32(data, rec + 8))
        .unwrap()
}

/// Noto Sans cut down to `.notdef` and `A`, and a copy of it whose `A` has
/// no contours. The two programs have the same length and differ only inside
/// `glyf`, past the first 256 bytes.
fn two_programs() -> (Vec<u8>, Vec<u8>) {
    let gid_a = ttf_parser::Face::parse(NOTO_SANS, 0)
        .unwrap()
        .glyph_index('A')
        .unwrap()
        .0;
    let with_a = subset_font(NOTO_SANS, &[0, gid_a]).unwrap().data;

    let loca = table(&with_a, b"loca");
    let long_loca = be16(&with_a, table(&with_a, b"head") + 50) == 1;
    let glyph_at = if long_loca {
        be32(&with_a, loca + 4 * gid_a as usize)
    } else {
        2 * be16(&with_a, loca + 2 * gid_a as usize)
    };
    let contours = table(&with_a, b"glyf") + glyph_at;
    assert!(contours >= 256);

    let mut empty_a = with_a.clone();
    empty_a[contours..contours + 2].copy_from_slice(&0u16.to_be_bytes());
    (with_a, empty_a)
}

fn hex(data: &[u8]) -> String {
    let mut s: String = data.iter().map(|b| format!("{b:02X}")).collect();
    s.push('>');
    s
}

/// A page showing `A` at 40 pt from x = 5 in `/F1` when `left`, and from
/// x = 55 in `/F2` when `right`, three times in turn. `/F1` is backed by the
/// program with `A`; `/F2` by the one without, unless `f2_has_a`.
fn doc(left: bool, right: bool, f2_has_a: bool) -> PdfDocument {
    let (with_a, empty_a) = two_programs();
    let mut content = String::new();
    for _ in 0..3 {
        if left {
            content.push_str("BT /F1 40 Tf 5 30 Td (A) Tj ET ");
        }
        if right {
            content.push_str("BT /F2 40 Tf 55 30 Td (A) Tj ET ");
        }
    }
    let font = |descriptor: u32| {
        format!(
            "<< /Type /Font /Subtype /TrueType /BaseFont /NotoSans /FirstChar 65 \
             /LastChar 65 /Widths [639] /FontDescriptor {descriptor} 0 R >>"
        )
    };
    let descriptor = |name: &str, program: u32| {
        format!(
            "<< /Type /FontDescriptor /FontName /{name} /Flags 32 \
             /FontBBox [0 0 1000 1000] /ItalicAngle 0 /Ascent 1000 /Descent 0 \
             /CapHeight 700 /StemV 80 /FontFile2 {program} 0 R >>"
        )
    };
    let (f1, f2) = (font(7), font(8));
    let (d1, d2) = (descriptor("WithA", 9), descriptor("EmptyA", 10));
    let (p1, p2) = (hex(&with_a), hex(if f2_has_a { &with_a } else { &empty_a }));
    page(
        "<< /Font << /F1 5 0 R /F2 6 0 R >> >>",
        &content,
        vec![
            Plain(&f1),
            Plain(&f2),
            Plain(&d1),
            Plain(&d2),
            Stream("/Filter /ASCIIHexDecode", &p1),
            Stream("/Filter /ASCIIHexDecode", &p2),
        ],
    )
}

/// RGBA rows of the page's left half (x < 50) and right half (x ≥ 50) at
/// 72 dpi.
fn halves(doc: &PdfDocument) -> (Vec<u8>, Vec<u8>) {
    let px = render_page_to_pixmap(doc, 0, &RenderOptions::default()).unwrap();
    let w = px.width as usize;
    let rows = px.data.chunks(4 * w);
    let left = rows.clone().flat_map(|row| row[..4 * w / 2].to_vec());
    let right = rows.flat_map(|row| row[4 * w / 2..].to_vec());
    (left.collect(), right.collect())
}

#[test]
fn each_font_draws_its_own_glyph_when_shown_in_turn() {
    let (left_alone, _) = halves(&doc(true, false, false));
    let (_, right_alone) = halves(&doc(false, true, false));
    let (left, right) = halves(&doc(true, true, false));
    assert_eq!(left, left_alone);
    assert_eq!(right, right_alone);
}

/// The two programs agree in length and in their first 256 bytes, and draw
/// `A` differently.
#[test]
fn the_two_programs_draw_different_glyphs() {
    let (with_a, empty_a) = two_programs();
    assert_eq!(with_a.len(), empty_a.len());
    assert_eq!(with_a[..256], empty_a[..256]);
    assert_ne!(with_a, empty_a);

    let (_, empty_a_drawn) = halves(&doc(false, true, false));
    let (_, a_drawn) = halves(&doc(false, true, true));
    assert_ne!(empty_a_drawn, a_drawn);
}
