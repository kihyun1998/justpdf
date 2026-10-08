//! `image::decode_image` applies a stream's whole filter chain and reads `/JBIG2Globals`.

use std::io::Write;

use justpdf_core::PdfDocument;
use justpdf_core::image::decode_image;
use justpdf_core::object::{IndirectRef, PdfDict, PdfObject};

/// A PDF file holding `objs` as objects 1, 2, … (object 1 the catalog).
fn pdf_file(objs: &[Vec<u8>]) -> Vec<u8> {
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
    out
}

/// A stream object with `dict_entries` and `data`.
fn stream_obj(dict_entries: &str, data: &[u8]) -> Vec<u8> {
    let mut o = format!("<< {dict_entries} /Length {} >>\nstream\n", data.len()).into_bytes();
    o.extend_from_slice(data);
    o.extend_from_slice(b"\nendstream");
    o
}

/// A one-page document whose object 4 is `globals_obj` (when given).
fn doc_with(globals_obj: Option<&[u8]>) -> PdfDocument {
    let mut objs: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] >>".to_vec(),
    ];
    objs.extend(globals_obj.map(<[u8]>::to_vec));
    PdfDocument::from_bytes(pdf_file(&objs)).unwrap()
}

fn name(n: &[u8]) -> PdfObject {
    PdfObject::Name(n.to_vec())
}

fn image_dict(w: i64, h: i64, cs: &[u8], filter: PdfObject) -> PdfDict {
    let mut d = PdfDict::new();
    d.insert(b"Type".to_vec(), name(b"XObject"));
    d.insert(b"Subtype".to_vec(), name(b"Image"));
    d.insert(b"Width".to_vec(), PdfObject::Integer(w));
    d.insert(b"Height".to_vec(), PdfObject::Integer(h));
    d.insert(b"ColorSpace".to_vec(), name(cs));
    d.insert(b"BitsPerComponent".to_vec(), PdfObject::Integer(8));
    d.insert(b"Filter".to_vec(), filter);
    d
}

fn flate(data: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

/// An 8×8 red JPEG.
fn red_jpeg() -> Vec<u8> {
    let rgb: Vec<u8> = (0..64).flat_map(|_| [255u8, 0, 0]).collect();
    let mut buf = std::io::Cursor::new(Vec::new());
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 90)
        .encode(&rgb, 8, 8, image::ExtendedColorType::Rgb8)
        .unwrap();
    buf.into_inner()
}

#[test]
fn a_flate_wrapped_jpeg_decodes_like_the_bare_jpeg() {
    let doc = doc_with(None);
    let jpeg = red_jpeg();
    let bare = decode_image(
        &jpeg,
        &image_dict(8, 8, b"DeviceRGB", name(b"DCTDecode")),
        &doc,
    )
    .unwrap();
    let chain = PdfObject::Array(vec![name(b"FlateDecode"), name(b"DCTDecode")]);
    let wrapped =
        decode_image(&flate(&jpeg), &image_dict(8, 8, b"DeviceRGB", chain), &doc).unwrap();
    assert_eq!(
        (wrapped.width, wrapped.height, wrapped.components),
        (8, 8, 3)
    );
    assert_eq!(wrapped.data, bare.data);
}

#[test]
fn a_raw_image_behind_two_filters_decodes() {
    let doc = doc_with(None);
    let pixels: Vec<u8> = (0..4u8).flat_map(|i| [i * 60, 255 - i * 60, 7]).collect();
    let hex: Vec<u8> = flate(&pixels)
        .iter()
        .flat_map(|b| format!("{b:02X}").into_bytes())
        .chain(*b">")
        .collect();
    let chain = PdfObject::Array(vec![name(b"ASCIIHexDecode"), name(b"FlateDecode")]);
    let img = decode_image(&hex, &image_dict(2, 2, b"DeviceRGB", chain), &doc).unwrap();
    assert_eq!(img.data, pixels);
}

/// An embedded JBIG2 stream for a blank 8×8 page: page information, end of page.
fn blank_jbig2_page() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&0u32.to_be_bytes());
    b.extend_from_slice(&[48, 0x00, 1]);
    b.extend_from_slice(&19u32.to_be_bytes());
    b.extend_from_slice(&8u32.to_be_bytes());
    b.extend_from_slice(&8u32.to_be_bytes());
    b.extend_from_slice(&[0; 8]);
    b.push(0);
    b.extend_from_slice(&0u16.to_be_bytes());
    b.extend_from_slice(&1u32.to_be_bytes());
    b.extend_from_slice(&[49, 0x00, 1]);
    b.extend_from_slice(&0u32.to_be_bytes());
    b
}

fn jbig2_dict(globals: Option<u32>) -> PdfDict {
    let mut d = image_dict(8, 8, b"DeviceGray", name(b"JBIG2Decode"));
    d.insert(b"BitsPerComponent".to_vec(), PdfObject::Integer(1));
    if let Some(n) = globals {
        let mut parms = PdfDict::new();
        parms.insert(
            b"JBIG2Globals".to_vec(),
            PdfObject::Reference(IndirectRef {
                obj_num: n,
                gen_num: 0,
            }),
        );
        d.insert(b"DecodeParms".to_vec(), PdfObject::Dict(parms));
    }
    d
}

#[test]
fn a_jbig2_page_without_globals_decodes() {
    let doc = doc_with(None);
    let img = decode_image(&blank_jbig2_page(), &jbig2_dict(None), &doc).unwrap();
    assert_eq!((img.width, img.height), (8, 8));
    assert!(img.data.iter().all(|&p| p == 0xFF));
}

#[test]
fn jbig2_globals_that_do_not_resolve_are_an_error() {
    let doc = doc_with(None);
    assert!(decode_image(&blank_jbig2_page(), &jbig2_dict(Some(9)), &doc).is_err());
}

#[test]
fn jbig2_globals_that_are_not_a_stream_are_an_error() {
    let doc = doc_with(Some(b"<< /Type /Foo >>"));
    assert!(decode_image(&blank_jbig2_page(), &jbig2_dict(Some(4)), &doc).is_err());
}

#[test]
fn a_flate_compressed_globals_stream_is_read() {
    let doc = doc_with(Some(&stream_obj("/Filter /FlateDecode", &flate(&[]))));
    let img = decode_image(&blank_jbig2_page(), &jbig2_dict(Some(4)), &doc).unwrap();
    assert_eq!((img.width, img.height), (8, 8));
}

#[test]
fn jbig2_globals_are_read_from_the_codec_filters_parms() {
    let doc = doc_with(None);
    let mut d = jbig2_dict(Some(9));
    let parms = d.get(b"DecodeParms").unwrap().clone();
    d.insert(
        b"Filter".to_vec(),
        PdfObject::Array(vec![name(b"FlateDecode"), name(b"JBIG2Decode")]),
    );
    d.insert(
        b"DecodeParms".to_vec(),
        PdfObject::Array(vec![PdfObject::Null, parms]),
    );
    assert!(decode_image(&flate(&blank_jbig2_page()), &d, &doc).is_err());

    // The same globals under the Flate filter's slot do not apply to JBIG2.
    let mut d = jbig2_dict(Some(9));
    let parms = d.get(b"DecodeParms").unwrap().clone();
    d.insert(
        b"Filter".to_vec(),
        PdfObject::Array(vec![name(b"FlateDecode"), name(b"JBIG2Decode")]),
    );
    d.insert(
        b"DecodeParms".to_vec(),
        PdfObject::Array(vec![parms, PdfObject::Null]),
    );
    assert!(decode_image(&flate(&blank_jbig2_page()), &d, &doc).is_ok());
}

/// A one-page PDF drawing the image XObject `Im0` over the page.
fn pdf_with_image(image_dict: &str, image_data: &[u8]) -> Vec<u8> {
    pdf_file(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R /Resources << /XObject << /Im0 5 0 R >> >> >>".to_vec(),
        stream_obj("", b"q 100 0 0 100 0 0 cm /Im0 Do Q"),
        stream_obj(image_dict, image_data),
    ])
}

/// A `size`×`size` JPEG of noise (large enough not to be skipped as small).
fn noise_jpeg(size: u32) -> Vec<u8> {
    let mut x: u32 = 1;
    let rgb: Vec<u8> = (0..size * size * 3)
        .map(|_| {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (x >> 16) as u8
        })
        .collect();
    let mut buf = std::io::Cursor::new(Vec::new());
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 95)
        .encode(&rgb, size, size, image::ExtendedColorType::Rgb8)
        .unwrap();
    buf.into_inner()
}

#[test]
fn compression_recompresses_a_flate_wrapped_jpeg() {
    use justpdf_core::writer::{CompressOptions, compress_pdf};
    let jpeg = noise_jpeg(200);
    let dict = "/Type /XObject /Subtype /Image /Width 200 /Height 200 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter [/FlateDecode /DCTDecode]";
    let pdf = pdf_with_image(dict, &flate(&jpeg));
    let mut options = CompressOptions::preset_high();
    options.max_image_dpi = None;
    let (_, stats) = compress_pdf(&pdf, &options).unwrap();
    assert_eq!(stats.images_found, 1);
    assert_eq!(stats.images_skipped, 0, "{stats:?}");
}

#[test]
fn the_jpeg_bytes_of_a_flate_wrapped_jpeg_are_the_jpeg() {
    use justpdf_core::image::extract_jpeg_bytes;
    let jpeg = red_jpeg();
    let chain = PdfObject::Array(vec![name(b"FlateDecode"), name(b"DCTDecode")]);
    let dict = image_dict(8, 8, b"DeviceRGB", chain);
    assert_eq!(extract_jpeg_bytes(&flate(&jpeg), &dict).unwrap(), jpeg);
    let bare = image_dict(8, 8, b"DeviceRGB", name(b"DCTDecode"));
    assert_eq!(extract_jpeg_bytes(&jpeg, &bare).unwrap(), jpeg);
}
