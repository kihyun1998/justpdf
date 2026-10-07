//! `/Rotate` turns the page clockwise when displayed, and the output takes the
//! visible box's size after rotation.

mod common;

use common::{Plain, Stream, pdf};
use justpdf_core::PdfDocument;
use justpdf_render::{RenderOptions, render_page, render_page_to_pixmap, render_page_to_svg};

const RED: [u8; 3] = [255, 0, 0];

/// A one-page document with the given page-dictionary entries and content.
fn doc(page_entries: &str, content: &str) -> PdfDocument {
    let page_dict =
        format!("<< /Type /Page /Parent 2 0 R {page_entries} /Resources << >> /Contents 4 0 R >>");
    pdf(&[
        Plain("<< /Type /Catalog /Pages 2 0 R >>"),
        Plain("<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        Plain(&page_dict),
        Stream("", content),
    ])
}

struct Image {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl Image {
    fn render(doc: &PdfDocument) -> Self {
        let px = render_page_to_pixmap(doc, 0, &RenderOptions::default()).unwrap();
        Self {
            width: px.width,
            height: px.height,
            data: px.data,
        }
    }

    fn rgb(&self, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * self.width + x) * 4) as usize;
        [self.data[i], self.data[i + 1], self.data[i + 2]]
    }

    /// The output corners (TL, TR, BL, BR) whose pixel 10px in from the corner is red.
    fn red_corners(&self) -> Vec<&'static str> {
        let (w, h) = (self.width - 1, self.height - 1);
        [
            ("top-left", 10, 10),
            ("top-right", w - 10, 10),
            ("bottom-left", 10, h - 10),
            ("bottom-right", w - 10, h - 10),
        ]
        .into_iter()
        .filter(|&(_, x, y)| self.rgb(x, y) == RED)
        .map(|(name, _, _)| name)
        .collect()
    }
}

/// A red 50x50 square in the top-left corner of a 200x200 page.
const TOP_LEFT_MARKER: &str = "1 0 0 rg 0 150 50 50 re f";

#[test]
fn rotation_turns_the_page_clockwise() {
    for (rotate, expected) in [
        (0, "top-left"),
        (90, "top-right"),
        (180, "bottom-right"),
        (270, "bottom-left"),
        (-90, "bottom-left"),
        (450, "top-right"),
    ] {
        let doc = doc(
            &format!("/MediaBox [0 0 200 200] /Rotate {rotate}"),
            TOP_LEFT_MARKER,
        );
        let img = Image::render(&doc);
        assert_eq!((img.width, img.height), (200, 200), "/Rotate {rotate}");
        assert_eq!(img.red_corners(), vec![expected], "/Rotate {rotate}");
    }
}

/// Width and height of a PNG, from its IHDR chunk.
fn png_size(png: &[u8]) -> (u32, u32) {
    assert_eq!(&png[1..4], b"PNG");
    let be = |b: &[u8]| u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
    (be(&png[16..20]), be(&png[20..24]))
}

#[test]
fn rotated_page_takes_the_rotated_size() {
    let doc = doc("/MediaBox [0 0 600 200] /Rotate 90", TOP_LEFT_MARKER);

    let img = Image::render(&doc);
    assert_eq!((img.width, img.height), (200, 600));
    assert_eq!(img.red_corners(), vec!["top-right"]);

    let png = render_page(&doc, 0, &RenderOptions::default()).unwrap();
    assert_eq!(png_size(&png), (200, 600));

    let svg = render_page_to_svg(&doc, 0).unwrap();
    assert!(
        svg.contains(r#"viewBox="0 0 200 600" width="200" height="600""#),
        "{}",
        &svg[..svg.len().min(300)]
    );

    let unrotated = doc_size("/MediaBox [0 0 600 200] /Rotate 180");
    assert_eq!(unrotated, (600, 200));
}

fn doc_size(page_entries: &str) -> (u32, u32) {
    let img = Image::render(&doc(page_entries, ""));
    (img.width, img.height)
}

#[test]
fn offset_crop_box_maps_exactly_onto_the_output() {
    // Blue everywhere outside the CropBox, a red marker at its top-left corner.
    let content = "0 0 1 rg 0 0 100 300 re f 300 0 100 300 re f 0 0 400 50 re f 0 250 400 50 re f \
                   1 0 0 rg 100 200 50 50 re f";
    for (rotate, expected) in [(0, "top-left"), (90, "top-right")] {
        let doc = doc(
            &format!("/MediaBox [0 0 400 300] /CropBox [100 50 300 250] /Rotate {rotate}"),
            content,
        );
        let img = Image::render(&doc);
        assert_eq!((img.width, img.height), (200, 200), "/Rotate {rotate}");
        assert_eq!(img.red_corners(), vec![expected], "/Rotate {rotate}");
        let blue = img
            .data
            .chunks_exact(4)
            .filter(|p| p[2] > 200 && p[0] < 50 && p[1] < 50)
            .count();
        assert_eq!(
            blue, 0,
            "/Rotate {rotate}: content outside the CropBox shows"
        );
    }
}
