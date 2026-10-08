//! Image XObjects and their masks reach `decode_image` as raw stream bytes,
//! decoded once, in the raster and SVG renderers.

mod common;

use common::{Obj, RED, Stream, WHITE, page, rgb};
use justpdf_core::writer::encode::make_stream;
use justpdf_render::render_page_to_svg;

/// `data` Flate-compressed, then ASCII-hex encoded (so it fits a text test PDF):
/// the stream carries `/Filter [/ASCIIHexDecode /FlateDecode …]`.
fn hex_flate(data: &[u8]) -> String {
    let (_, compressed) = make_stream(data, true);
    let mut s: String = compressed.iter().map(|b| format!("{b:02X}")).collect();
    s.push('>');
    s
}

fn solid(r: u8, g: u8, b: u8, n: usize) -> Vec<u8> {
    (0..n).flat_map(|_| [r, g, b]).collect()
}

/// A page drawing `/Im0` (object 5) over the whole page; further objects from 6.
fn image_page(image: Obj<'_>, rest: Vec<Obj<'_>>) -> justpdf_core::PdfDocument {
    let mut objs = vec![image];
    objs.extend(rest);
    page(
        "<< /XObject << /Im0 5 0 R >> >>",
        "q 100 0 0 100 0 0 cm /Im0 Do Q",
        objs,
    )
}

const RGB_8X8: &str =
    "/Type /XObject /Subtype /Image /Width 8 /Height 8 /ColorSpace /DeviceRGB /BitsPerComponent 8";

#[test]
fn a_flate_image_fills_its_area() {
    let data = hex_flate(&solid(255, 0, 0, 64));
    let dict = format!("{RGB_8X8} /Filter [/ASCIIHexDecode /FlateDecode]");
    let doc = image_page(Stream(&dict, &data), vec![]);
    assert_eq!(rgb(&doc, 50, 50), RED);
}

#[test]
fn a_flate_smask_of_zero_hides_the_image_and_of_255_shows_it() {
    let red = hex_flate(&solid(255, 0, 0, 64));
    let dict = format!("{RGB_8X8} /Filter [/ASCIIHexDecode /FlateDecode] /SMask 6 0 R");
    let smask_dict = "/Type /XObject /Subtype /Image /Width 8 /Height 8 /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter [/ASCIIHexDecode /FlateDecode]";

    let clear = hex_flate(&[0u8; 64]);
    let doc = image_page(Stream(&dict, &red), vec![Stream(smask_dict, &clear)]);
    assert_eq!(rgb(&doc, 50, 50), WHITE);

    let opaque = hex_flate(&[255u8; 64]);
    let doc = image_page(Stream(&dict, &red), vec![Stream(smask_dict, &opaque)]);
    assert_eq!(rgb(&doc, 50, 50), RED);
}

#[test]
fn a_flate_explicit_mask_masks_part_of_the_image() {
    let red = hex_flate(&solid(255, 0, 0, 64));
    let dict = format!("{RGB_8X8} /Filter [/ASCIIHexDecode /FlateDecode] /Mask 6 0 R");
    // Each row: the left four samples 0, the right four 1.
    let mask = hex_flate(&[0x0F; 8]);
    let mask_dict = "/Type /XObject /Subtype /Image /Width 8 /Height 8 /ImageMask true /Filter [/ASCIIHexDecode /FlateDecode]";
    let doc = image_page(Stream(&dict, &red), vec![Stream(mask_dict, &mask)]);
    assert_ne!(rgb(&doc, 25, 50), rgb(&doc, 75, 50));
}

#[test]
fn a_flate_stencil_mask_paints_only_its_marked_samples() {
    // Each row: the left four samples 0 (painted), the right four 1.
    let mask = hex_flate(&[0x0F; 8]);
    let dict = "/Type /XObject /Subtype /Image /Width 8 /Height 8 /ImageMask true /Filter [/ASCIIHexDecode /FlateDecode]";
    let doc = page(
        "<< /XObject << /Im0 5 0 R >> >>",
        "1 0 0 rg q 100 0 0 100 0 0 cm /Im0 Do Q",
        vec![Stream(dict, &mask)],
    );
    assert_eq!(rgb(&doc, 25, 50), RED);
    assert_eq!(rgb(&doc, 75, 50), WHITE);
}

fn red_jpeg() -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 95)
        .encode(&solid(255, 0, 0, 64), 8, 8, image::ExtendedColorType::Rgb8)
        .unwrap();
    buf.into_inner()
}

#[test]
fn a_flate_wrapped_jpeg_still_renders() {
    let data = hex_flate(&red_jpeg());
    let dict = format!("{RGB_8X8} /Filter [/ASCIIHexDecode /FlateDecode /DCTDecode]");
    let doc = image_page(Stream(&dict, &data), vec![]);
    let [r, g, b] = rgb(&doc, 50, 50);
    assert!(r > 240 && g < 20 && b < 20, "{:?}", [r, g, b]);
}

#[test]
fn the_svg_renderer_emits_a_flate_image() {
    let data = hex_flate(&solid(255, 0, 0, 64));
    let dict = format!("{RGB_8X8} /Filter [/ASCIIHexDecode /FlateDecode]");
    let doc = image_page(Stream(&dict, &data), vec![]);
    let svg = render_page_to_svg(&doc, 0).unwrap();
    assert!(svg.contains("<image"), "{svg}");
}
