//! Image samples become RGB through one core conversion that reads array
//! colour spaces (Indexed, ICCBased, Separation, DeviceN), `/BitsPerComponent`
//! and `/Decode` (#46). Each image is 8 × 1 (or 10 × 2 at 1 bpc): colour A in
//! its left half, colour B in its right half, drawn over the 100 × 100 page.

mod common;

use common::{Obj, Plain, RED, Stream, WHITE, page, rgb};
use justpdf_core::writer::compress::{CompressOptions, compress_pdf};
use justpdf_render::render_page_to_svg;

const BLUE: [u8; 3] = [0, 0, 255];
const BLACK: [u8; 3] = [0, 0, 0];

/// A page drawing image object 5 over the whole page; further objects from 6.
fn image_page(image: Obj<'_>, rest: Vec<Obj<'_>>) -> justpdf_core::PdfDocument {
    let mut objs = vec![image];
    objs.extend(rest);
    page(
        "<< /XObject << /Im0 5 0 R >> >>",
        "q 100 0 0 100 0 0 cm /Im0 Do Q",
        objs,
    )
}

/// An image dictionary `w` × `h`, with `entries` and ASCII-hex data.
fn image(w: u32, h: u32, entries: &str) -> String {
    format!(
        "/Type /XObject /Subtype /Image /Width {w} /Height {h} {entries} /Filter /ASCIIHexDecode"
    )
}

/// The left-half and right-half pixels.
fn halves(doc: &justpdf_core::PdfDocument) -> ([u8; 3], [u8; 3]) {
    (rgb(doc, 12, 50), rgb(doc, 88, 50))
}

/// `[n] * 4` then `[m] * 4`, as hex samples of `bytes` bytes each.
fn two_halves(a: &str, b: &str) -> String {
    format!("{}{}>", a.repeat(4), b.repeat(4))
}

#[test]
fn an_rgb_image_is_unchanged() {
    let data = two_halves("FF0000", "0000FF");
    let dict = image(8, 1, "/ColorSpace /DeviceRGB /BitsPerComponent 8");
    let doc = image_page(Stream(&dict, &data), vec![]);
    assert_eq!(halves(&doc), (RED, BLUE));
}

// ---- Indexed ----------------------------------------------------------------

#[test]
fn an_indexed_image_with_an_inline_lookup() {
    let data = two_halves("00", "01");
    let dict = image(
        8,
        1,
        "/ColorSpace [/Indexed /DeviceRGB 1 <FF00000000FF>] /BitsPerComponent 8",
    );
    let doc = image_page(Stream(&dict, &data), vec![]);
    assert_eq!(halves(&doc), (RED, BLUE));
}

#[test]
fn an_indexed_image_with_a_lookup_stream() {
    let data = two_halves("00", "01");
    let dict = image(
        8,
        1,
        "/ColorSpace [/Indexed /DeviceRGB 1 6 0 R] /BitsPerComponent 8",
    );
    let doc = image_page(
        Stream(&dict, &data),
        vec![Stream("/Filter /ASCIIHexDecode", "FF00000000FF>")],
    );
    assert_eq!(halves(&doc), (RED, BLUE));
}

#[test]
fn an_indexed_colour_space_named_by_reference() {
    let data = two_halves("00", "01");
    let dict = image(8, 1, "/ColorSpace 6 0 R /BitsPerComponent 8");
    let doc = image_page(
        Stream(&dict, &data),
        vec![Plain("[/Indexed /DeviceRGB 1 <FF00000000FF>]")],
    );
    assert_eq!(halves(&doc), (RED, BLUE));
}

// ---- bits per component -------------------------------------------------------

#[test]
fn a_one_bit_gray_image_with_rows_padded_to_a_byte() {
    // 10 × 2: five black then five white samples per row, each row two bytes.
    let dict = image(10, 2, "/ColorSpace /DeviceGray /BitsPerComponent 1");
    let doc = image_page(Stream(&dict, "07C007C0>"), vec![]);
    assert_eq!(halves(&doc), (BLACK, WHITE));
}

#[test]
fn a_four_bit_gray_image() {
    // White then black: read as 8-bit samples, the four bytes would cover
    // only the left half, and the missing right half would be white.
    let dict = image(8, 1, "/ColorSpace /DeviceGray /BitsPerComponent 4");
    let doc = image_page(Stream(&dict, "FFFF0000>"), vec![]);
    assert_eq!(halves(&doc), (WHITE, BLACK));
}

#[test]
fn a_sixteen_bit_rgb_image() {
    let data = two_halves("FFFF00000000", "00000000FFFF");
    let dict = image(8, 1, "/ColorSpace /DeviceRGB /BitsPerComponent 16");
    let doc = image_page(Stream(&dict, &data), vec![]);
    assert_eq!(halves(&doc), (RED, BLUE));
}

// ---- /Decode ------------------------------------------------------------------

#[test]
fn decode_inverts_a_gray_image() {
    let data = two_halves("00", "FF");
    let dict = image(
        8,
        1,
        "/ColorSpace /DeviceGray /BitsPerComponent 8 /Decode [1 0]",
    );
    let doc = image_page(Stream(&dict, &data), vec![]);
    assert_eq!(halves(&doc), (WHITE, BLACK));
}

// ---- ICCBased, Separation, DeviceN --------------------------------------------

#[test]
fn an_icc_based_image_uses_its_component_count() {
    let data = two_halves("00", "FF");
    let dict = image(8, 1, "/ColorSpace [/ICCBased 6 0 R] /BitsPerComponent 8");
    let doc = image_page(
        Stream(&dict, &data),
        vec![Stream("/N 1 /Alternate /DeviceGray", "")],
    );
    assert_eq!(halves(&doc), (BLACK, WHITE));
}

/// Tint 0 → no ink, tint 1 → the CMYK red `0 1 1 0`.
const RED_TINT: &str = "<< /FunctionType 2 /Domain [0 1] /C0 [0 0 0 0] /C1 [0 1 1 0] /N 1 >>";

#[test]
fn a_separation_image_goes_through_its_tint_transform() {
    let data = two_halves("00", "FF");
    let dict = image(
        8,
        1,
        &format!("/ColorSpace [/Separation /Spot /DeviceCMYK {RED_TINT}] /BitsPerComponent 8"),
    );
    let doc = image_page(Stream(&dict, &data), vec![]);
    assert_eq!(halves(&doc), (WHITE, RED));
}

#[test]
fn a_device_n_image_goes_through_its_tint_transform() {
    let data = two_halves("00", "FF");
    let dict = image(
        8,
        1,
        &format!("/ColorSpace [/DeviceN [/Spot] /DeviceCMYK {RED_TINT}] /BitsPerComponent 8"),
    );
    let doc = image_page(Stream(&dict, &data), vec![]);
    assert_eq!(halves(&doc), (WHITE, RED));
}

// ---- SVG ----------------------------------------------------------------------

/// The SVG of a page drawing `dict` with `data`.
fn svg_of(dict: &str, data: &str) -> String {
    render_page_to_svg(&image_page(Stream(dict, data), vec![]), 0).unwrap()
}

#[test]
fn svg_embeds_an_indexed_image_as_its_rgb_equivalent() {
    let rgb_svg = svg_of(
        &image(8, 1, "/ColorSpace /DeviceRGB /BitsPerComponent 8"),
        &two_halves("FF0000", "0000FF"),
    );
    let indexed_svg = svg_of(
        &image(
            8,
            1,
            "/ColorSpace [/Indexed /DeviceRGB 1 <FF00000000FF>] /BitsPerComponent 8",
        ),
        &two_halves("00", "01"),
    );
    assert_eq!(indexed_svg, rgb_svg);
}

#[test]
fn svg_embeds_a_sixteen_bit_image_as_its_eight_bit_equivalent() {
    let eight = svg_of(
        &image(8, 1, "/ColorSpace /DeviceRGB /BitsPerComponent 8"),
        &two_halves("FF0000", "0000FF"),
    );
    let sixteen = svg_of(
        &image(8, 1, "/ColorSpace /DeviceRGB /BitsPerComponent 16"),
        &two_halves("FFFF00000000", "00000000FFFF"),
    );
    assert_eq!(sixteen, eight);
}

// ---- compression ----------------------------------------------------------------

fn compress(doc_bytes: &[u8]) -> Vec<u8> {
    let options = CompressOptions {
        skip_below_bytes: 0,
        ..CompressOptions::preset_medium()
    };
    compress_pdf(doc_bytes, &options).unwrap().0
}

/// The bytes of a one-page PDF drawing image object 5 (`dict`, `data`).
fn image_pdf_bytes(dict: &str, data: &str) -> Vec<u8> {
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] \
         /Resources << /XObject << /Im0 5 0 R >> >> /Contents 4 0 R >>"
            .to_string(),
        "<< /Length 31 >>\nstream\nq 100 0 0 100 0 0 cm /Im0 Do Q\nendstream".to_string(),
        format!(
            "<< {dict} /Length {} >>\nstream\n{data}\nendstream",
            data.len()
        ),
    ];
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(b"xref\n0 6\n0000000000 65535 f \n");
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    out
}

#[test]
fn compressing_a_sixteen_bit_image_does_not_panic() {
    let bytes = image_pdf_bytes(
        &image(8, 1, "/ColorSpace /DeviceRGB /BitsPerComponent 16"),
        &two_halves("FFFF00000000", "00000000FFFF"),
    );
    let out = compress(&bytes);
    assert!(justpdf_core::PdfDocument::from_bytes(out).is_ok());
}

#[test]
fn a_compressed_indexed_image_keeps_its_colours() {
    // 64 × 64: index 0 (red) in the left 32 columns, index 1 (blue) in the right.
    let row = format!("{}{}", "00".repeat(32), "01".repeat(32));
    let data = format!("{}>", row.repeat(64));
    let bytes = image_pdf_bytes(
        &image(
            64,
            64,
            "/ColorSpace [/Indexed /DeviceRGB 1 <FF00000000FF>] /BitsPerComponent 8",
        ),
        &data,
    );
    let out = compress(&bytes);
    assert_ne!(out, bytes, "the image was recompressed");
    let doc = justpdf_core::PdfDocument::from_bytes(out).unwrap();
    let (left, right) = halves(&doc);
    assert!(left[0] > 200 && left[2] < 60, "red half: {left:?}");
    assert!(right[2] > 200 && right[0] < 60, "blue half: {right:?}");
}
