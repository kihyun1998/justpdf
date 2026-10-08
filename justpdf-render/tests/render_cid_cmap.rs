//! The raster and SVG renderers read a Type0 font's codes through its
//! embedded encoding CMap: widths and `/CIDToGIDMap` are looked up by CID.

mod common;

use common::{Plain, RED, Stream, WHITE, page, rgb};
use justpdf_core::PdfDocument;
use justpdf_core::font::subset::subset_font;
use justpdf_core::ttf_parser;
use justpdf_render::{RenderOptions, render_page_to_pixmap, render_page_to_svg};

const NOTO_SANS: &[u8] = include_bytes!("../../justpdf-core/tests/fixtures/NotoSans-Regular.ttf");

/// Maps code `<0101>` to CID 300.
const CODE_0101_TO_CID_300: &str = "/CIDInit /ProcSet findresource begin 12 dict begin begincmap \
     1 begincodespacerange <0000> <FFFF> endcodespacerange \
     1 begincidchar <0101> 300 endcidchar \
     endcmap CMapName currentdict /CMap defineresource pop end end";

/// A page showing the hex string `shown` in red at 10 pt from (10, 40) in a
/// Type0 font whose `/Encoding` is the CMap stream `cmap`, over a
/// CIDFontType2 with no font program (each glyph a box as wide as its
/// width) whose dictionary ends with `entries`.
fn show(cmap: &str, entries: &str, shown: &str) -> PdfDocument {
    let cid = format!(
        "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /CIDTest \
         /CIDSystemInfo << /Registry (Adobe) /Ordering (Test) /Supplement 0 >> {entries} >>"
    );
    page(
        "<< /Font << /F1 5 0 R >> >>",
        &format!("BT /F1 10 Tf 1 0 0 rg 10 40 Td <{shown}> Tj ET"),
        vec![
            Plain(
                "<< /Type /Font /Subtype /Type0 /BaseFont /CIDTest /Encoding 7 0 R \
                 /DescendantFonts [6 0 R] >>",
            ),
            Plain(&cid),
            Stream("/Type /CMap", cmap),
        ],
    )
}

#[test]
fn a_raster_box_is_as_wide_as_the_mapped_cids_width() {
    // <0101> is CID 300, 2000 units: its box spans x 10–30. Code 0x101 itself
    // is 500 units wide, which would end the box at x 15.
    let doc = show(CODE_0101_TO_CID_300, "/W [257 [500] 300 [2000]]", "0101");
    assert_eq!(rgb(&doc, 25, 58), RED);
    assert_eq!(rgb(&doc, 35, 58), WHITE);
}

#[test]
fn an_svg_placeholder_is_as_wide_as_the_mapped_cids_width() {
    let svg = render_page_to_svg(
        &show(CODE_0101_TO_CID_300, "/W [257 [500] 300 [2000]]", "0101"),
        0,
    )
    .unwrap();
    assert!(svg.contains("M0 -2 L20 -2"), "{svg}");
}

fn hex(data: &[u8]) -> String {
    let mut s: String = data.iter().map(|b| format!("{b:02X}")).collect();
    s.push('>');
    s
}

/// A page showing `shown` at 60 pt from (10, 20) in a Type0 font over a
/// CIDFontType2 backed by Noto Sans cut down to `.notdef` and `A`, whose
/// `/CIDToGIDMap` sends CID 300 to `A` and every other CID to `.notdef`.
/// `/Encoding` is `encoding`; `extra` follow as objects 10 on.
fn glyph_page(encoding: &str, shown: &str, extra: Vec<common::Obj>) -> PdfDocument {
    let gid_a = ttf_parser::Face::parse(NOTO_SANS, 0)
        .unwrap()
        .glyph_index('A')
        .unwrap()
        .0;
    let program = hex(&subset_font(NOTO_SANS, &[0, gid_a]).unwrap().data);
    let mut map = vec![0u8; 2 * 301];
    map[600..602].copy_from_slice(&gid_a.to_be_bytes());
    let map = hex(&map);

    let font = format!(
        "<< /Type /Font /Subtype /Type0 /BaseFont /NotoSans /Encoding {encoding} \
         /DescendantFonts [6 0 R] >>"
    );
    let mut objects = vec![
        Plain(&font),
        Plain(
            "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /NotoSans \
             /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
             /DW 1000 /FontDescriptor 7 0 R /CIDToGIDMap 9 0 R >>",
        ),
        Plain(
            "<< /Type /FontDescriptor /FontName /NotoSans /Flags 32 \
             /FontBBox [0 0 1000 1000] /ItalicAngle 0 /Ascent 1000 /Descent 0 \
             /CapHeight 700 /StemV 80 /FontFile2 8 0 R >>",
        ),
        Stream("/Filter /ASCIIHexDecode", &program),
        Stream("/Filter /ASCIIHexDecode", &map),
    ];
    objects.extend(extra);
    let content = format!("BT /F1 60 Tf 10 20 Td <{shown}> Tj ET");
    page("<< /Font << /F1 5 0 R >> >>", &content, objects)
}

fn pixels(doc: &PdfDocument) -> Vec<u8> {
    render_page_to_pixmap(doc, 0, &RenderOptions::default())
        .unwrap()
        .data
}

#[test]
fn the_raster_glyph_comes_from_cid_to_gid_map_at_the_mapped_cid() {
    // Code <0101> maps to CID 300, which /CIDToGIDMap sends to A: the page
    // matches showing CID 300 directly through Identity-H, and differs from
    // showing CID 0x101 (.notdef).
    let mapped = pixels(&glyph_page(
        "10 0 R",
        "0101",
        vec![Stream("/Type /CMap", CODE_0101_TO_CID_300)],
    ));
    let direct_300 = pixels(&glyph_page("/Identity-H", "012C", vec![]));
    let direct_0101 = pixels(&glyph_page("/Identity-H", "0101", vec![]));
    assert_ne!(direct_300, direct_0101);
    assert!(mapped == direct_300, "the mapped code drew another glyph");
}

#[test]
fn a_cid_past_65535_draws_notdef_instead_of_a_wrapped_cid() {
    // <0101> maps to CID 65836, which as a u16 would be CID 300 (A).
    let cmap = CODE_0101_TO_CID_300.replace("<0101> 300", "<0101> 65836");
    let mapped = pixels(&glyph_page(
        "10 0 R",
        "0101",
        vec![Stream("/Type /CMap", &cmap)],
    ));
    let notdef = pixels(&glyph_page("/Identity-H", "0101", vec![]));
    assert!(
        mapped == notdef,
        "a CID past 65535 drew a glyph other than .notdef"
    );
}
