//! `DecodedImage::to_rgb8`: samples unpacked by `/BitsPerComponent`, mapped
//! through `/Decode` and converted from array colour spaces (#46).

use std::io::Write;

use justpdf_core::PdfDocument;
use justpdf_core::image::decode_image;
use justpdf_core::object::{IndirectRef, PdfObject};

/// A one-page document holding `extra` as objects 4, 5, … and, as the last
/// object, an image stream with `dict` entries and `data`. Returns the
/// document and the image's object number.
fn doc_with_image(extra: &[Vec<u8>], dict: &str, data: &[u8]) -> (PdfDocument, u32) {
    let mut objs: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] >>".to_vec(),
    ];
    objs.extend(extra.iter().cloned());
    let mut image = format!(
        "<< /Type /XObject /Subtype /Image {dict} /Length {} >>\nstream\n",
        data.len()
    )
    .into_bytes();
    image.extend_from_slice(data);
    image.extend_from_slice(b"\nendstream");
    objs.push(image);
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        writeln!(out, "{} 0 obj", i + 1).unwrap();
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    write!(out, "xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).unwrap();
    for off in offsets {
        writeln!(out, "{off:010} 00000 n ").unwrap();
    }
    write!(
        out,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objs.len() + 1
    )
    .unwrap();
    let n = objs.len() as u32;
    (PdfDocument::from_bytes(out).unwrap(), n)
}

/// The RGB pixels of the image `dict` / `data`, with `extra` objects from 4.
fn rgb(extra: &[&str], dict: &str, data: &[u8]) -> Vec<[u8; 3]> {
    let extra: Vec<Vec<u8>> = extra.iter().map(|o| o.as_bytes().to_vec()).collect();
    let (doc, n) = doc_with_image(&extra, dict, data);
    let PdfObject::Stream { dict, data } = doc
        .resolve(&IndirectRef {
            obj_num: n,
            gen_num: 0,
        })
        .unwrap()
    else {
        panic!("not a stream")
    };
    let image = decode_image(&data, &dict, &doc).unwrap();
    image
        .to_rgb8()
        .chunks_exact(3)
        .map(|p| [p[0], p[1], p[2]])
        .collect()
}

const RED: [u8; 3] = [255, 0, 0];
const BLUE: [u8; 3] = [0, 0, 255];
const BLACK: [u8; 3] = [0, 0, 0];
const WHITE: [u8; 3] = [255, 255, 255];

// ---- Indexed ----------------------------------------------------------------

const INDEXED_2: &str =
    "/Width 2 /Height 1 /BitsPerComponent 8 /ColorSpace [/Indexed /DeviceRGB 1 <FF00000000FF>]";

#[test]
fn indexed_with_an_inline_lookup() {
    assert_eq!(rgb(&[], INDEXED_2, &[0, 1]), [RED, BLUE]);
}

#[test]
fn indexed_with_a_lookup_stream() {
    let dict = "/Width 2 /Height 1 /BitsPerComponent 8 /ColorSpace [/Indexed /DeviceRGB 1 4 0 R]";
    let stream = "<< /Length 13 /Filter /ASCIIHexDecode >>\nstream\nFF00000000FF>\nendstream";
    assert_eq!(rgb(&[stream], dict, &[0, 1]), [RED, BLUE]);
}

#[test]
fn indexed_named_by_reference() {
    let dict = "/Width 2 /Height 1 /BitsPerComponent 8 /ColorSpace 4 0 R";
    assert_eq!(
        rgb(&["[/Indexed /DeviceRGB 1 <FF00000000FF>]"], dict, &[0, 1]),
        [RED, BLUE]
    );
}

#[test]
fn an_index_past_hival_is_clamped() {
    assert_eq!(rgb(&[], INDEXED_2, &[0, 7]), [RED, BLUE]);
}

#[test]
fn decode_maps_the_index_range() {
    // /Decode is in index units: at 1 bpc, [1 0] swaps indices 0 and 1.
    let dict = "/Width 2 /Height 1 /BitsPerComponent 1 /Decode [1 0] \
                /ColorSpace [/Indexed /DeviceRGB 1 <FF00000000FF>]";
    assert_eq!(rgb(&[], dict, &[0b0100_0000]), [BLUE, RED]);
    // At 8 bpc the same [1 0] maps 0 to 1 and 1 to 1 - 1/255, which rounds
    // to 1: both blue.
    let dict = format!("{INDEXED_2} /Decode [1 0]");
    assert_eq!(rgb(&[], &dict, &[0, 1]), [BLUE, BLUE]);
}

#[test]
fn a_two_bit_indexed_image() {
    // Four indices 0..3 in one byte; a four-entry palette.
    let dict = "/Width 4 /Height 1 /BitsPerComponent 2 \
                /ColorSpace [/Indexed /DeviceRGB 3 <FF00000000FF00FF00FFFFFF>]";
    assert_eq!(
        rgb(&[], dict, &[0b00_01_10_11]),
        [RED, BLUE, [0, 255, 0], WHITE]
    );
}

// ---- bits per component -------------------------------------------------------

#[test]
fn one_bit_gray_rows_start_on_a_byte_boundary() {
    // 10 × 2: each row is 2 bytes; row 0 is 1100000000, row 1 0000000011.
    let dict = "/Width 10 /Height 2 /BitsPerComponent 1 /ColorSpace /DeviceGray";
    let px = rgb(
        &[],
        dict,
        &[0b1100_0000, 0b0000_0000, 0b0000_0000, 0b1100_0000],
    );
    let row0: Vec<_> = px[..10].to_vec();
    let row1: Vec<_> = px[10..].to_vec();
    assert_eq!(row0, [vec![WHITE; 2], vec![BLACK; 8]].concat());
    assert_eq!(row1, [vec![BLACK; 8], vec![WHITE; 2]].concat());
}

#[test]
fn two_and_four_bit_gray() {
    let two = "/Width 4 /Height 1 /BitsPerComponent 2 /ColorSpace /DeviceGray";
    assert_eq!(
        rgb(&[], two, &[0b00_01_10_11]),
        [BLACK, [85, 85, 85], [170, 170, 170], WHITE]
    );
    let four = "/Width 2 /Height 1 /BitsPerComponent 4 /ColorSpace /DeviceGray";
    assert_eq!(rgb(&[], four, &[0xF0]), [WHITE, BLACK]);
}

#[test]
fn sixteen_bit_gray_and_rgb() {
    let gray = "/Width 2 /Height 1 /BitsPerComponent 16 /ColorSpace /DeviceGray";
    assert_eq!(rgb(&[], gray, &[0xFF, 0xFF, 0x00, 0x00]), [WHITE, BLACK]);
    let rgb16 = "/Width 2 /Height 1 /BitsPerComponent 16 /ColorSpace /DeviceRGB";
    let data = [0xFF, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF];
    assert_eq!(rgb(&[], rgb16, &data), [RED, BLUE]);
}

#[test]
fn decode_inverts_gray() {
    let dict = "/Width 2 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceGray /Decode [1 0]";
    assert_eq!(rgb(&[], dict, &[0, 255]), [WHITE, BLACK]);
}

#[test]
fn eight_bit_device_images_convert_as_before() {
    let cmyk = "/Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceCMYK";
    // (1 - 0.2)(1 - 0.5) × 255 = 102 for c = 51, k = 128 (truncated).
    let c = ((1.0 - 51.0 / 255.0) * (1.0 - 128.0 / 255.0) * 255.0) as u8;
    assert_eq!(rgb(&[], cmyk, &[51, 0, 0, 128])[0][0], c);
    let gray = "/Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceGray";
    assert_eq!(rgb(&[], gray, &[77]), [[77, 77, 77]]);
}

// ---- ICCBased, Separation, DeviceN --------------------------------------------

#[test]
fn icc_based_n1_and_n4() {
    let n1 = "<< /N 1 /Length 0 >>\nstream\n\nendstream";
    let dict = "/Width 2 /Height 1 /BitsPerComponent 8 /ColorSpace [/ICCBased 4 0 R]";
    assert_eq!(rgb(&[n1], dict, &[0, 255]), [BLACK, WHITE]);
    let n4 = "<< /N 4 /Length 0 >>\nstream\n\nendstream";
    let dict = "/Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace [/ICCBased 4 0 R]";
    assert_eq!(rgb(&[n4], dict, &[0, 255, 255, 0]), [RED]);
}

#[test]
fn icc_based_follows_its_alternate() {
    let stream = "<< /N 3 /Alternate /DeviceRGB /Length 0 >>\nstream\n\nendstream";
    let dict = "/Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace [/ICCBased 4 0 R]";
    assert_eq!(rgb(&[stream], dict, &[0, 0, 255]), [BLUE]);
}

const RED_TINT: &str = "<< /FunctionType 2 /Domain [0 1] /C0 [0 0 0 0] /C1 [0 1 1 0] /N 1 >>";

#[test]
fn separation_through_a_type_2_tint_transform() {
    let dict = format!(
        "/Width 2 /Height 1 /BitsPerComponent 8 /ColorSpace [/Separation /Spot /DeviceCMYK {RED_TINT}]"
    );
    assert_eq!(rgb(&[], &dict, &[0, 255]), [WHITE, RED]);
}

#[test]
fn device_n_through_a_type_2_tint_transform() {
    let dict = format!(
        "/Width 2 /Height 1 /BitsPerComponent 8 /ColorSpace [/DeviceN [/Spot] /DeviceCMYK {RED_TINT}]"
    );
    assert_eq!(rgb(&[], &dict, &[0, 255]), [WHITE, RED]);
}

#[test]
fn a_tint_transform_by_reference() {
    let dict = "/Width 2 /Height 1 /BitsPerComponent 8 \
                /ColorSpace [/Separation /Spot /DeviceCMYK 4 0 R]";
    assert_eq!(rgb(&[RED_TINT], dict, &[0, 255]), [WHITE, RED]);
}

// ---- Bits per component (#459) ---------------------------------------------

/// `values` packed `bpc` bits each, most significant bit first, in one row.
fn pack(values: &[u32], bpc: u32) -> Vec<u8> {
    let mut out = vec![0u8; (values.len() * bpc as usize).div_ceil(8)];
    for (i, &v) in values.iter().enumerate() {
        for b in 0..bpc as usize {
            if v >> (bpc as usize - 1 - b) & 1 == 1 {
                let bit = i * bpc as usize + b;
                out[bit / 8] |= 0x80 >> (bit % 8);
            }
        }
    }
    out
}

#[test]
fn every_depth_from_1_to_16_reads_samples_that_cross_a_byte() {
    for bpc in 1..=16u32 {
        let max = (1u32 << bpc) - 1;
        let values = [max, 0, max / 3, max, max / 2];
        let dict = format!("/Width 5 /Height 1 /BitsPerComponent {bpc} /ColorSpace /DeviceGray");
        let expected: Vec<[u8; 3]> = values
            .iter()
            .map(|&v| {
                let g = (v as f64 / max as f64 * 255.0).round() as u8;
                [g, g, g]
            })
            .collect();
        assert_eq!(rgb(&[], &dict, &pack(&values, bpc)), expected, "bpc {bpc}");
    }
}

#[test]
fn each_row_of_an_odd_depth_starts_on_a_byte() {
    // 3 samples of 3 bits: 9 bits, padded to 2 bytes per row.
    let mut data = pack(&[7, 0, 7], 3);
    data.extend(pack(&[0, 7, 0], 3));
    let dict = "/Width 3 /Height 2 /BitsPerComponent 3 /ColorSpace /DeviceGray";
    assert_eq!(
        rgb(&[], dict, &data),
        [WHITE, BLACK, WHITE, BLACK, WHITE, BLACK]
    );
}

#[test]
fn a_depth_of_zero_or_none_reads_as_8() {
    for depth in ["/BitsPerComponent 0", ""] {
        let dict = format!("/Width 2 /Height 1 {depth} /ColorSpace /DeviceGray");
        assert_eq!(rgb(&[], &dict, &[255, 0]), [WHITE, BLACK], "{depth:?}");
    }
}

#[test]
fn a_depth_below_0_or_above_16_is_refused() {
    for bpc in [-1, 17, 32] {
        let dict = format!("/Width 2 /Height 1 /BitsPerComponent {bpc} /ColorSpace /DeviceGray");
        let (doc, n) = doc_with_image(&[], &dict, &[255, 0, 255, 0, 255, 0, 255, 0]);
        let PdfObject::Stream { dict, data } = doc
            .resolve(&IndirectRef {
                obj_num: n,
                gen_num: 0,
            })
            .unwrap()
        else {
            panic!("not a stream")
        };
        assert!(decode_image(&data, &dict, &doc).is_err(), "bpc {bpc}");
    }
}
