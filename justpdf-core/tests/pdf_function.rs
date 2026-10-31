//! `PdfFunction::resolve`: functions read through the document (references
//! resolved, streams decoded), sampled (Type 0) functions, and evaluation that
//! never panics (#47), and the bound on resolving shared and cyclic
//! sub-functions (#437).

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
    assert!(f.is_none());
}

#[test]
fn a_reversed_domain_or_range_does_not_panic() {
    let f = function(&[
        b"<< /FunctionType 2 /Domain [1 0] /Range [1 0] /C0 [0] /C1 [1] /N 1 >>".to_vec(),
    ])
    .unwrap();
    assert_eq!(f.evaluate(&[0.5]).len(), 1);
}

#[test]
fn an_array_of_functions_concatenates_their_outputs() {
    let f = function(&[
        b"[<< /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 1 >> 5 0 R]".to_vec(),
        b"<< /FunctionType 2 /Domain [0 1] /C0 [1] /C1 [0] /N 1 >>".to_vec(),
    ])
    .unwrap();
    assert!(close(&f.evaluate(&[0.25]), &[0.25, 0.75]));
}

/// Object 4 of a document holding `objs`, resolved on another thread: fails
/// after 10 s, or at once if resolving panics, instead of hanging the run.
fn function_within_10s(objs: Vec<Vec<u8>>) -> Option<PdfFunction> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(function(&objs));
    });
    rx.recv_timeout(std::time::Duration::from_secs(10))
        .expect("resolve panicked or did not finish within 10 s")
}

/// A stitching function over [0 1] whose `/Functions` are `subs`.
fn stitching(subs: &[String]) -> Vec<u8> {
    let k = subs.len();
    let bounds: Vec<String> = (1..k).map(|i| format!("{}", i as f64 / k as f64)).collect();
    format!(
        "<< /FunctionType 3 /Domain [0 1] /Bounds [{}] /Encode [{}] /Functions [{}] >>",
        bounds.join(" "),
        "0 1 ".repeat(k),
        subs.join(" ")
    )
    .into_bytes()
}

fn obj_ref(n: usize) -> String {
    format!("{n} 0 R")
}

const CONSTANT_QUARTER: &[u8] = b"<< /FunctionType 2 /Domain [0 1] /C0 [0.25] /C1 [0.25] /N 1 >>";

#[test]
fn a_stitching_function_listing_itself_many_times_fails_promptly() {
    let f = function_within_10s(vec![stitching(&vec![obj_ref(4); 20])]);
    assert!(f.is_none());
}

#[test]
fn a_two_function_cycle_fails() {
    let f = function_within_10s(vec![stitching(&[obj_ref(5)]), stitching(&[obj_ref(4)])]);
    assert!(f.is_none());
}

#[test]
fn a_cycle_through_a_function_array_fails() {
    // 4 is an array listing the stitching function 5, which lists 4 again.
    let f = function_within_10s(vec![
        format!("[{}]", obj_ref(5)).into_bytes(),
        stitching(&[obj_ref(4)]),
    ]);
    assert!(f.is_none());
}

#[test]
fn a_tree_sharing_its_sub_functions_resolves_each_once() {
    // Objects 4..=11 are stitching functions each listing the next 20 times;
    // object 12, at the depth cap, is the constant 0.25. Unshared, that is
    // 20^8 leaves.
    let mut objs: Vec<Vec<u8>> = (5..=12)
        .map(|next| stitching(&vec![obj_ref(next); 20]))
        .collect();
    objs.push(CONSTANT_QUARTER.to_vec());
    let f = function_within_10s(objs).expect("the shared tree resolves");
    for x in [0.0, 0.33, 0.9, 1.0] {
        assert!(close(&f.evaluate(&[x]), &[0.25]), "at {x}");
    }
}

#[test]
fn a_shared_function_past_the_depth_cap_fails_as_it_would_unshared() {
    // 4 lists 5 and 11. 5..=9 chain to the leaf 10 (depth 6 from 4). 11..=13
    // chain to 5, which puts the leaf at depth 9, past the cap of 8.
    let mut objs = vec![stitching(&[obj_ref(5), obj_ref(11)])];
    objs.extend((6..=10).map(|next| stitching(&[obj_ref(next)])));
    objs.push(CONSTANT_QUARTER.to_vec());
    objs.extend((12..=14).map(|next| stitching(&[obj_ref(if next == 14 { 5 } else { next })])));
    assert_eq!(objs.len(), 10);
    assert!(function_within_10s(objs).is_none());
}

#[test]
fn a_stitching_function_with_an_invalid_sub_function_fails() {
    let f = function(&[
        stitching(&[obj_ref(5), obj_ref(6)]),
        CONSTANT_QUARTER.to_vec(),
        b"<< /FunctionType 9 >>".to_vec(),
    ]);
    assert!(f.is_none());
}

#[test]
fn a_stitching_function_takes_at_most_256_sub_functions() {
    let ok = function(&[stitching(&vec![obj_ref(5); 256]), CONSTANT_QUARTER.to_vec()]);
    assert!(close(
        &ok.expect("256 sub-functions").evaluate(&[0.5]),
        &[0.25]
    ));
    let over = function(&[stitching(&vec![obj_ref(5); 257]), CONSTANT_QUARTER.to_vec()]);
    assert!(over.is_none());
}

#[test]
fn a_stitching_function_without_sub_functions_fails() {
    assert!(function(&[stitching(&[])]).is_none());
    let missing = b"<< /FunctionType 3 /Domain [0 1] /Bounds [] /Encode [0 1] >>".to_vec();
    assert!(function(&[missing]).is_none());
}

#[test]
fn an_array_inside_a_function_array_fails() {
    let nested = format!("[{} [{}]]", obj_ref(5), obj_ref(5)).into_bytes();
    assert!(function(&[nested, CONSTANT_QUARTER.to_vec()]).is_none());
    let as_sub = format!("[{}]", obj_ref(6)).into_bytes();
    assert!(
        function(&[
            as_sub,
            CONSTANT_QUARTER.to_vec(),
            format!("[{}]", obj_ref(5)).into_bytes()
        ])
        .is_none()
    );
    let in_stitching = stitching(&[format!("[{}]", obj_ref(5))]);
    assert!(function(&[in_stitching, CONSTANT_QUARTER.to_vec()]).is_none());
}

#[test]
fn parse_applies_the_stitching_and_array_rules() {
    let leaf = String::from_utf8(CONSTANT_QUARTER.to_vec()).unwrap();
    let parse = |bytes: Vec<u8>| {
        let doc = doc(&[bytes]);
        let obj = doc
            .resolve(&IndirectRef {
                obj_num: 4,
                gen_num: 0,
            })
            .unwrap();
        PdfFunction::parse(&obj)
    };
    assert!(parse(stitching(std::slice::from_ref(&leaf))).is_some());
    assert!(parse(stitching(&[leaf.clone(), "<< /FunctionType 9 >>".into()])).is_none());
    assert!(parse(stitching(&[])).is_none());
    assert!(parse(format!("[{leaf} [{leaf}]]").into_bytes()).is_none());
}
