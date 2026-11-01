//! A signature `sign_pdf` writes verifies over the file it is written into:
//! the `/ByteRange` hole is exactly the `/Contents` string (#408, #37), and the
//! zero-padded `/Contents` decodes (#409).
//!
//! The key and certificate under `tests/fixtures` are a throwaway RSA-2048
//! key and a self-signed certificate made with OpenSSL for these tests.

use justpdf_core::object::IndirectRef;
use justpdf_core::sign::{SignatureInfo, SigningOptions, sign_pdf, verify_signature};

const KEY: &[u8] = include_bytes!("fixtures/sign_test_key.der");
const CERT: &[u8] = include_bytes!("fixtures/sign_test_cert.der");

fn signed() -> Vec<u8> {
    let pdf = std::fs::read("tests/fixtures/minimal.pdf").unwrap();
    sign_pdf(&pdf, KEY, &[CERT], &SigningOptions::default()).unwrap()
}

/// The last `/ByteRange` array in `pdf`.
fn byte_range(pdf: &[u8]) -> [i64; 4] {
    let key = b"/ByteRange [";
    let start = pdf.windows(key.len()).rposition(|w| w == key).unwrap() + key.len();
    let end = start + pdf[start..].iter().position(|&b| b == b']').unwrap();
    let values: Vec<i64> = std::str::from_utf8(&pdf[start..end])
        .unwrap()
        .split_whitespace()
        .map(|v| v.parse().unwrap())
        .collect();
    values.try_into().unwrap()
}

/// The bytes of the hex digits in `pdf[from..to]`, without its `<` and `>`
/// where they are inside.
fn hex_string(pdf: &[u8], from: usize, to: usize) -> Vec<u8> {
    let hex = pdf[from..to].strip_prefix(b"<").unwrap_or(&pdf[from..to]);
    let hex = hex.strip_suffix(b">").unwrap_or(hex);
    hex.chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

/// `contents` cut to its first DER element, by the length in its header.
fn first_der_element(contents: &[u8]) -> Vec<u8> {
    let (len, header) = match contents[1] {
        n if n < 0x80 => (n as usize, 2),
        n => {
            let count = (n & 0x7f) as usize;
            let len = contents[2..2 + count]
                .iter()
                .fold(0usize, |acc, &b| acc << 8 | b as usize);
            (len, 2 + count)
        }
    };
    contents[..header + len].to_vec()
}

fn info(byte_range: [i64; 4], contents_raw: Vec<u8>) -> SignatureInfo {
    SignatureInfo {
        field_name: "Signature1".into(),
        field_ref: IndirectRef {
            obj_num: 0,
            gen_num: 0,
        },
        signer_name: None,
        signing_time: None,
        reason: None,
        location: None,
        contact_info: None,
        filter: b"Adobe.PPKLite".to_vec(),
        sub_filter: b"adbe.pkcs7.detached".to_vec(),
        byte_range: byte_range.to_vec(),
        contents_raw,
        cert_chain: Vec::new(),
    }
}

#[test]
fn the_byte_range_hole_is_exactly_the_contents_string() {
    let pdf = signed();
    let [start, hole, after, rest] = byte_range(&pdf);
    assert_eq!(start, 0);
    assert_eq!(after as usize + rest as usize, pdf.len());
    assert_eq!(pdf[hole as usize], b'<');
    assert_eq!(pdf[after as usize - 1], b'>');
    assert!(
        pdf[hole as usize - 11..hole as usize].ends_with(b"/Contents "),
        "{:?}",
        String::from_utf8_lossy(&pdf[hole as usize - 11..hole as usize])
    );
}

#[test]
fn the_digest_covers_the_file_as_written() {
    let pdf = signed();
    let range = byte_range(&pdf);
    let contents = hex_string(&pdf, range[1] as usize, range[2] as usize);
    let result = verify_signature(&pdf, &info(range, first_der_element(&contents))).unwrap();
    assert!(result.digest_valid, "{:?}", result.validity);
    assert!(result.signature_valid, "{:?}", result.validity);
}

#[test]
fn a_zero_padded_contents_verifies_like_the_bare_der() {
    let pdf = signed();
    let range = byte_range(&pdf);
    let contents = hex_string(&pdf, range[1] as usize, range[2] as usize);
    assert!(contents.len() > first_der_element(&contents).len());
    let padded = verify_signature(&pdf, &info(range, contents.clone())).unwrap();
    assert!(padded.digest_valid, "{:?}", padded.validity);
    assert!(padded.signature_valid, "{:?}", padded.validity);
}
