//! `PdfFunction::resolve`: functions read through the document (references
//! resolved, streams decoded), sampled (Type 0) functions, and evaluation that
//! never panics (#47).

use std::io::Write;

use justpdf_core::PdfDocument;
use justpdf_core::function::PdfFunction;
use justpdf_core::object::{IndirectRef, PdfObject};
use justpdf_core::writer::encode_flate;

/// A one-page document holding `objs` as objects 4, 5, …
fn doc(objs: &[Vec<u8>]) -> PdfDocument {
    let mut all: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] >>".to_vec(),
    ];
    all.extend(objs.iter().cloned());
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in all.iter().enumerate() {
        offsets.push(out.len());
        writeln!(out, "{} 0 obj", i + 1).unwrap();
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    write!(out, "xref\n0 {}\n0000000000 65535 f \n", all.len() + 1).unwrap();
    for off in offsets {
        writeln!(out, "{off:010} 00000 n ").unwrap();
    }
    write!(
        out,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        all.len() + 1
    )
    .unwrap();
    PdfDocument::from_bytes(out).unwrap()
}

fn stream(dict: &str, data: &[u8]) -> Vec<u8> {
    let mut o = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
    o.extend_from_slice(data);
    o.extend_from_slice(b"\nendstream");
    o
}

/// Object 4 of a document holding `objs`, read with `PdfFunction::resolve`.
fn function(objs: &[Vec<u8>]) -> Option<PdfFunction> {
    let doc = doc(objs);
    PdfFunction::resolve(
        &PdfObject::Reference(IndirectRef {
            obj_num: 4,
            gen_num: 0,
        }),
        &doc,
    )
}

fn close(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3)
}

// ---- Type 0 -------------------------------------------------------------------

#[test]
fn a_sampled_function_interpolates_between_its_samples() {
    // 1 input → 3 outputs, 8-bit: f(0) = (0, 128, 255), f(1) = (255, 128, 0).
    let f = function(&[stream(
        "/FunctionType 0 /Domain [0 1] /Range [0 1 0 1 0 1] /Size [2] /BitsPerSample 8",
        &[0, 128, 255, 255, 128, 0],
    )])
    .unwrap();
    assert!(close(&f.evaluate(&[0.25]), &[0.25, 128.0 / 255.0, 0.75]));
    assert!(close(&f.evaluate(&[0.0]), &[0.0, 128.0 / 255.0, 1.0]));
}

#[test]
fn a_two_input_sampled_function_is_multilinear() {
    // f(0,0) = 0, f(1,0) = 1, f(0,1) = 1, f(1,1) = 0: the first input varies
    // fastest. At (0.25, 0.25): 0.25 × 0.75 + 0.75 × 0.25 = 0.375.
    let f = function(&[stream(
        "/FunctionType 0 /Domain [0 1 0 1] /Range [0 1] /Size [2 2] /BitsPerSample 8",
        &[0, 255, 255, 0],
    )])
    .unwrap();
    assert!(close(&f.evaluate(&[0.25, 0.25]), &[0.375]));
    assert!(close(&f.evaluate(&[1.0, 0.0]), &[1.0]));
}

#[test]
fn one_and_sixteen_bit_samples() {
    let one = function(&[stream(
        "/FunctionType 0 /Domain [0 1] /Range [0 1] /Size [2] /BitsPerSample 1",
        &[0b0100_0000],
    )])
    .unwrap();
    assert!(close(&one.evaluate(&[0.0]), &[0.0]));
    assert!(close(&one.evaluate(&[1.0]), &[1.0]));
    let sixteen = function(&[stream(
        "/FunctionType 0 /Domain [0 1] /Range [0 1] /Size [2] /BitsPerSample 16",
        &[0, 0, 0xFF, 0xFF],
    )])
    .unwrap();
    assert!(close(&sixteen.evaluate(&[0.5]), &[0.5]));
}

#[test]
fn encode_and_decode_map_inputs_and_samples() {
    // /Encode [1 0] reverses the table; /Decode [0 2] doubles the output.
    let f = function(&[stream(
        "/FunctionType 0 /Domain [0 1] /Range [0 2] /Size [2] /BitsPerSample 8 \
         /Encode [1 0] /Decode [0 2]",
        &[0, 255],
    )])
    .unwrap();
    assert!(close(&f.evaluate(&[0.0]), &[2.0]));
    assert!(close(&f.evaluate(&[1.0]), &[0.0]));
}

#[test]
fn a_truncated_sample_table_does_not_panic() {
    let f = function(&[stream(
        "/FunctionType 0 /Domain [0 1] /Range [0 1 0 1 0 1] /Size [4] /BitsPerSample 8",
        &[255],
    )])
    .unwrap();
    assert_eq!(f.evaluate(&[0.9]).len(), 3);
}

#[test]
fn order_3_falls_back_to_linear() {
    let f = function(&[stream(
        "/FunctionType 0 /Domain [0 1] /Range [0 1] /Size [2] /BitsPerSample 8 /Order 3",
        &[0, 255],
    )])
    .unwrap();
    assert!(close(&f.evaluate(&[0.5]), &[0.5]));
}

#[test]
fn a_filtered_sampled_function_is_decoded() {
    let data = encode_flate(&[0, 255]).unwrap();
    let f = function(&[stream(
        "/FunctionType 0 /Domain [0 1] /Range [0 1] /Size [2] /BitsPerSample 8 /Filter /FlateDecode",
        &data,
    )])
    .unwrap();
    assert!(close(&f.evaluate(&[0.25]), &[0.25]));
}

// ---- Type 4 -------------------------------------------------------------------

#[test]
fn a_flate_filtered_postscript_function_matches_its_unfiltered_twin() {
    let code = b"{ dup 0.5 mul }";
    let dict = "/FunctionType 4 /Domain [0 1] /Range [0 1 0 1]";
    let plain = function(&[stream(dict, code)]).unwrap();
    let filtered = function(&[stream(
        &format!("{dict} /Filter /FlateDecode"),
        &encode_flate(code).unwrap(),
    )])
    .unwrap();
    assert!(close(&filtered.evaluate(&[0.8]), &plain.evaluate(&[0.8])));
    assert!(close(&filtered.evaluate(&[0.8]), &[0.8, 0.4]));
}

#[test]
fn more_inputs_than_domain_pairs_do_not_panic() {
    let f = function(&[stream(
        "/FunctionType 4 /Domain [0 1] /Range [0 1]",
        b"{ add }",
    )])
    .unwrap();
    assert_eq!(f.evaluate(&[0.25, 0.5]).len(), 1);
    let no_domain = function(&[stream("/FunctionType 4 /Range [0 1]", b"{ add }")]).unwrap();
    assert_eq!(no_domain.evaluate(&[0.25, 0.5]).len(), 1);
}

// ---- Type 3 -------------------------------------------------------------------

#[test]
fn stitching_sub_functions_by_reference() {
    // [0, 0.5) → 5 0 R (constant 0.2), [0.5, 1] → 6 0 R (constant 0.9).
    let f = function(&[
        b"<< /FunctionType 3 /Domain [0 1] /Bounds [0.5] /Encode [0 1 0 1] \
           /Functions [5 0 R 6 0 R] >>"
            .to_vec(),
        b"<< /FunctionType 2 /Domain [0 1] /C0 [0.2] /C1 [0.2] /N 1 >>".to_vec(),
        b"<< /FunctionType 2 /Domain [0 1] /C0 [0.9] /C1 [0.9] /N 1 >>".to_vec(),
    ])
    .unwrap();
    assert!(close(&f.evaluate(&[0.25]), &[0.2]));
    assert!(close(&f.evaluate(&[0.75]), &[0.9]));
}

#[test]
fn a_stitching_function_naming_itself_stops() {
    let f = function(&[
        b"<< /FunctionType 3 /Domain [0 1] /Bounds [] /Encode [0 1] \
           /Functions [4 0 R] >>"
            .to_vec(),
    ]);
    // Resolved or not, it must return rather than recurse without end.
    if let Some(f) = f {
        let _ = f.evaluate(&[0.5]);
    }
}

#[test]
fn a_reversed_domain_or_range_does_not_panic() {
    let f = function(&[
        b"<< /FunctionType 2 /Domain [1 0] /Range [1 0] /C0 [0] /C1 [1] /N 1 >>".to_vec(),
    ])
    .unwrap();
    assert_eq!(f.evaluate(&[0.5]).len(), 1);
}
