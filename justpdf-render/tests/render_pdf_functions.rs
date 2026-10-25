//! Functions the renderer reads through the document (#47): function-based
//! shadings and tint transforms given as sampled (Type 0) or filtered Type 4
//! functions paint their colours instead of a fallback.

mod common;

use common::{Obj, RED, Stream, page, rgb};
use justpdf_core::writer::encode_flate;

/// `data` as an ASCII-hex string ending in `>`.
fn hex(data: &[u8]) -> String {
    let mut s: String = data.iter().map(|b| format!("{b:02X}")).collect();
    s.push('>');
    s
}

/// A page painting `/Sh0`, a function-based shading over the page whose
/// `/Function` is object 5 (`function`).
fn function_shading(function: Obj<'_>) -> justpdf_core::PdfDocument {
    page(
        "<< /Shading << /Sh0 << /ShadingType 1 /ColorSpace /DeviceRGB \
         /Domain [0 1 0 1] /Matrix [100 0 0 100 0 0] /Function 5 0 R >> >> >>",
        "/Sh0 sh",
        vec![function],
    )
}

#[test]
fn a_function_based_shading_with_a_sampled_function() {
    // 2 inputs → 3 outputs, every sample red.
    let samples = hex(&[255, 0, 0].repeat(4));
    let dict = "/FunctionType 0 /Domain [0 1 0 1] /Range [0 1 0 1 0 1] /Size [2 2] \
                /BitsPerSample 8 /Filter /ASCIIHexDecode";
    let doc = function_shading(Stream(dict, &samples));
    assert_eq!(rgb(&doc, 50, 50), RED);
}

#[test]
fn a_function_based_shading_with_a_flate_postscript_function() {
    // (x, y) → (1, 0, 0): `pop pop 1 0 0`.
    let code = hex(&encode_flate(b"{ pop pop 1 0 0 }").unwrap());
    let dict = "/FunctionType 4 /Domain [0 1 0 1] /Range [0 1 0 1 0 1] \
                /Filter [/ASCIIHexDecode /FlateDecode]";
    let doc = function_shading(Stream(dict, &code));
    assert_eq!(rgb(&doc, 50, 50), RED);
}

#[test]
fn a_separation_with_a_sampled_tint_transform() {
    // Tint 0 → CMYK 0 0 0 0, tint 1 → CMYK 0 1 1 0 (red), as a Type 0
    // function referenced from the colour space.
    let samples = hex(&[0, 0, 0, 0, 0, 255, 255, 0]);
    let doc = page(
        "<< /ColorSpace << /CS0 [/Separation /Spot /DeviceCMYK 5 0 R] >> >>",
        "/CS0 cs 1 scn 0 0 100 100 re f",
        vec![Stream(
            "/FunctionType 0 /Domain [0 1] /Range [0 1 0 1 0 1 0 1] /Size [2] \
             /BitsPerSample 8 /Filter /ASCIIHexDecode",
            &samples,
        )],
    );
    assert_eq!(rgb(&doc, 50, 50), RED);
}
