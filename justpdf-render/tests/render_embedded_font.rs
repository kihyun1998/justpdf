//! Hangul drawn through `DocumentBuilder::add_font` renders as its glyphs:
//! not blank, and not the `.notdef` the same number of missing characters draws.

use justpdf_core::PdfDocument;
use justpdf_core::font::FontProgram;
use justpdf_core::writer::{DocumentBuilder, PageBuilder};
use justpdf_render::{RenderOptions, render_page_to_pixmap};

const HANGUL: &[u8] =
    include_bytes!("../../justpdf-core/tests/fixtures/JustpdfTestHangul-Regular.ttf");

/// RGBA pixels of a 200 × 100 pt page drawing `text` at 48 pt in the Hangul
/// fixture, at 72 dpi.
fn render(text: &str) -> Vec<u8> {
    let mut doc = DocumentBuilder::new();
    let font = doc.add_font(&FontProgram::parse(HANGUL.to_vec(), 0).unwrap());
    let mut page = PageBuilder::new(200.0, 100.0);
    page.begin_text();
    page.set_font(&font, 48.0);
    page.move_to(10.0, 30.0);
    page.show_text(text);
    page.end_text();
    doc.add_page(page);
    let pdf = PdfDocument::from_bytes(doc.build().unwrap()).unwrap();
    render_page_to_pixmap(&pdf, 0, &RenderOptions::default())
        .unwrap()
        .data
}

fn dark_pixels(rgba: &[u8]) -> usize {
    rgba.chunks(4).filter(|p| p[0] < 128).count()
}

#[test]
fn hangul_renders_as_glyphs_not_blank_and_not_notdef() {
    let hangul = render("한글");
    let notdef = render("😀🙂");
    assert!(
        dark_pixels(&hangul) > 200,
        "{} dark pixels",
        dark_pixels(&hangul)
    );
    assert_ne!(hangul, notdef);
    assert_ne!(render("한"), render("글"));
}
