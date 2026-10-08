//! Rendering decodes each embedded font program once per document, shared by
//! every page, render call and thread.

mod common;

use common::{Plain, Stream, pdf};
use justpdf_core::PdfDocument;
use justpdf_render::{RenderOptions, render_page};

/// URW NimbusSans Regular, a bare CFF (SIL OFL 1.1, `fixtures/OFL-URW.txt`).
const NIMBUS_SANS: &[u8] = include_bytes!("fixtures/NimbusSans-Regular.cff");

fn hex(data: &[u8]) -> String {
    let mut s: String = data.iter().map(|b| format!("{b:02X}")).collect();
    s.push('>');
    s
}

/// `pages` pages that each show `H` in one Type1C font (object 4) whose
/// `/FontFile3` (object 6) holds `program` behind `filter`.
fn doc(pages: usize, program: &str, filter: &str) -> PdfDocument {
    let kids: Vec<String> = (0..pages).map(|i| format!("{} 0 R", 7 + i)).collect();
    let pages_dict = format!(
        "<< /Type /Pages /Kids [{}] /Count {pages} >>",
        kids.join(" ")
    );
    let page = "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] \
                /Resources << /Font << /F1 4 0 R >> >> /Contents 3 0 R >>";
    let stream_dict = format!("/Subtype /Type1C /Filter {filter}");
    let mut objects = vec![
        Plain("<< /Type /Catalog /Pages 2 0 R >>"),
        Plain(&pages_dict),
        Stream("", "BT /F1 50 Tf 10 20 Td (H) Tj (H) Tj ET"),
        Plain(
            "<< /Type /Font /Subtype /Type1 /BaseFont /NimbusSans-Regular \
             /Encoding /WinAnsiEncoding /FontDescriptor 5 0 R >>",
        ),
        Plain(
            "<< /Type /FontDescriptor /FontName /NimbusSans-Regular /Flags 32 \
             /FontBBox [0 0 1000 1000] /ItalicAngle 0 /Ascent 1000 /Descent 0 \
             /CapHeight 729 /StemV 80 /FontFile3 6 0 R >>",
        ),
        Stream(&stream_dict, program),
    ];
    objects.extend((0..pages).map(|_| Plain(page)));
    pdf(&objects)
}

fn nimbus(pages: usize) -> PdfDocument {
    doc(pages, &hex(NIMBUS_SANS), "/ASCIIHexDecode")
}

fn render_all(doc: &PdfDocument, pages: usize) -> Vec<Vec<u8>> {
    (0..pages)
        .map(|i| render_page(doc, i, &RenderOptions::default()).unwrap())
        .collect()
}

#[test]
fn pages_sharing_a_font_decode_it_once() {
    let doc = nimbus(3);
    render_all(&doc, 3);
    assert_eq!(doc.font_program_decodes(), 1);
}

#[test]
fn rendering_a_page_twice_decodes_its_font_once() {
    let doc = nimbus(1);
    let first = render_all(&doc, 1);
    let second = render_all(&doc, 1);
    assert_eq!(first, second);
    assert_eq!(doc.font_program_decodes(), 1);
}

#[test]
fn a_font_that_fails_to_decode_is_tried_once() {
    let doc = doc(3, "not flate data", "/FlateDecode");
    render_all(&doc, 3);
    assert_eq!(doc.font_program_decodes(), 1);
}

#[cfg(feature = "parallel")]
#[test]
fn parallel_rendering_shares_the_cache_and_matches_sequential() {
    let sequential = render_all(&nimbus(4), 4);
    let doc = nimbus(4);
    let parallel: Vec<Vec<u8>> =
        justpdf_render::render_all_pages_parallel(&doc, &RenderOptions::default())
            .into_iter()
            .map(Result::unwrap)
            .collect();
    assert_eq!(parallel, sequential);
    assert_eq!(doc.font_program_decodes(), 1);
}
