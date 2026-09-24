//! Integration tests for `justpdf encrypt`'s trailer `/ID`.

use std::path::{Path, PathBuf};
use std::process::Command;

use justpdf_core::{PdfDocument, PdfObject};

const SOURCE_ID: [u8; 16] = *b"justpdf-src-id!!";
const SOURCE_CHANGING_ID: [u8; 16] = *b"justpdf-changing";

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_justpdf"))
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

/// `compressible.pdf` with `/ID [<first> <second>]` added to its last trailer.
fn source_with_id(first: &[u8], second: &[u8], tag: &str) -> PathBuf {
    let bytes = std::fs::read(fixture("compressible.pdf")).unwrap();
    let at = bytes
        .windows(10)
        .rposition(|w| w == b"trailer\n<<")
        .unwrap()
        + 10;
    let mut out = bytes[..at].to_vec();
    out.extend_from_slice(format!(" /ID [<{}> <{}>]", hex(first), hex(second)).as_bytes());
    out.extend_from_slice(&bytes[at..]);
    let path = std::env::temp_dir().join(format!("justpdf_cli_encrypt_src_{tag}.pdf"));
    std::fs::write(&path, out).unwrap();
    path
}

/// Run `justpdf encrypt` on `input` and return the output's trailer `/ID`.
fn encrypt_and_read_id(input: &Path, tag: &str) -> Vec<Vec<u8>> {
    let out = std::env::temp_dir().join(format!("justpdf_cli_encrypt_{tag}.pdf"));
    let _ = std::fs::remove_file(&out);
    let status = bin()
        .arg("encrypt")
        .arg(input)
        .args(["--user-password", "user", "--owner-password", "owner", "-o"])
        .arg(&out)
        .status()
        .unwrap();
    assert!(status.success(), "encrypt failed for {tag}");

    let mut doc = PdfDocument::open(&out).unwrap();
    doc.authenticate(b"user").unwrap();
    match doc.trailer().get(b"ID") {
        Some(PdfObject::Array(arr)) => arr
            .iter()
            .map(|o| match o {
                PdfObject::String(s) => s.clone(),
                other => panic!("/ID element is not a string: {other:?}"),
            })
            .collect(),
        other => panic!("{tag}: trailer /ID missing: {other:?}"),
    }
}

#[test]
fn encrypt_keeps_the_source_permanent_id() {
    let input = source_with_id(&SOURCE_ID, &SOURCE_CHANGING_ID, "id");
    let source = PdfDocument::open(&input).unwrap();
    assert!(
        source.trailer().get(b"ID").is_some(),
        "fixture must carry /ID"
    );

    let id = encrypt_and_read_id(&input, "kept");
    assert_eq!(id[0], SOURCE_ID);
}

#[test]
fn encrypt_without_source_id_gets_a_fresh_one() {
    let input = fixture("compressible.pdf");
    assert!(
        PdfDocument::open(&input)
            .unwrap()
            .trailer()
            .get(b"ID")
            .is_none()
    );

    let a = encrypt_and_read_id(&input, "fresh_a");
    let b = encrypt_and_read_id(&input, "fresh_b");
    assert_eq!(a[0].len(), 16);
    assert_ne!(a[0], b[0], "two encryptions share one /ID");
}

#[test]
fn encrypt_with_empty_source_id_gets_a_fresh_one() {
    let input = source_with_id(b"", b"", "empty_id");
    assert!(
        PdfDocument::open(&input)
            .unwrap()
            .trailer()
            .get(b"ID")
            .is_some()
    );

    let a = encrypt_and_read_id(&input, "empty_a");
    let b = encrypt_and_read_id(&input, "empty_b");
    assert_eq!(a[0].len(), 16);
    assert_ne!(a[0], b[0], "two encryptions share one /ID");
}
