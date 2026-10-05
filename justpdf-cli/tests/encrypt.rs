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

/// `compressible.pdf` with `/ID [<first> <second>]` added to its last trailer,
/// written into `dir`.
fn source_with_id(dir: &Path, first: &[u8], second: &[u8]) -> PathBuf {
    let bytes = std::fs::read(fixture("compressible.pdf")).unwrap();
    let at = bytes
        .windows(10)
        .rposition(|w| w == b"trailer\n<<")
        .unwrap()
        + 10;
    let mut out = bytes[..at].to_vec();
    out.extend_from_slice(format!(" /ID [<{}> <{}>]", hex(first), hex(second)).as_bytes());
    out.extend_from_slice(&bytes[at..]);
    let path = dir.join("source.pdf");
    std::fs::write(&path, out).unwrap();
    path
}

/// `/Info /Title` of `doc`, if any.
fn info_title(doc: &PdfDocument) -> Option<Vec<u8>> {
    let info = match doc.trailer().get(b"Info") {
        Some(PdfObject::Reference(r)) => r.clone(),
        _ => return None,
    };
    match doc.resolve(&info).ok()? {
        PdfObject::Dict(d) => match d.get(b"Title") {
            Some(PdfObject::String(s)) => Some(s.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// Run `justpdf encrypt` on `input` into `dir/{tag}.pdf`; assert the output
/// keeps the input's `/Info /Title`, and return the output's trailer `/ID`.
fn encrypt_and_read_id(dir: &Path, input: &Path, tag: &str) -> Vec<Vec<u8>> {
    let source_title = info_title(&PdfDocument::open(input).unwrap());
    assert!(
        source_title.is_some(),
        "{tag}: fixture must carry /Info /Title"
    );
    let out = dir.join(format!("{tag}.pdf"));
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
    assert_eq!(info_title(&doc), source_title, "{tag}: /Info /Title lost");
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
    let dir = tempfile::tempdir().unwrap();
    let input = source_with_id(dir.path(), &SOURCE_ID, &SOURCE_CHANGING_ID);
    let source = PdfDocument::open(&input).unwrap();
    assert!(
        source.trailer().get(b"ID").is_some(),
        "fixture must carry /ID"
    );

    let id = encrypt_and_read_id(dir.path(), &input, "kept");
    assert_eq!(id[0], SOURCE_ID);
    assert_eq!(id[1].len(), 16);
    assert_ne!(id[1], SOURCE_ID, "changing identifier not updated");
    assert_ne!(id[1], SOURCE_CHANGING_ID, "changing identifier not updated");
}

#[test]
fn encrypt_without_source_id_gets_a_fresh_one() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture("compressible.pdf");
    assert!(
        PdfDocument::open(&input)
            .unwrap()
            .trailer()
            .get(b"ID")
            .is_none()
    );

    let a = encrypt_and_read_id(dir.path(), &input, "fresh_a");
    let b = encrypt_and_read_id(dir.path(), &input, "fresh_b");
    assert_eq!(a[0].len(), 16);
    assert_eq!(a[0], a[1], "a file without /ID is written as new");
    assert_ne!(a[0], b[0], "two encryptions share one /ID");
}

#[test]
fn encrypt_with_empty_source_id_gets_a_fresh_one() {
    let dir = tempfile::tempdir().unwrap();
    let input = source_with_id(dir.path(), b"", b"");
    assert!(
        PdfDocument::open(&input)
            .unwrap()
            .trailer()
            .get(b"ID")
            .is_some()
    );

    let a = encrypt_and_read_id(dir.path(), &input, "empty_a");
    let b = encrypt_and_read_id(dir.path(), &input, "empty_b");
    assert_eq!(a[0].len(), 16);
    assert_eq!(a[0], a[1], "an empty /ID is treated as absent");
    assert_ne!(a[0], b[0], "two encryptions share one /ID");
}

#[test]
fn encrypt_refuses_an_encrypted_input() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("once.pdf");
    let status = bin()
        .arg("encrypt")
        .arg(fixture("compressible.pdf"))
        .args(["--user-password", "user", "--owner-password", "owner", "-o"])
        .arg(&first)
        .status()
        .unwrap();
    assert!(status.success());

    let second = dir.path().join("twice.pdf");
    let output = bin()
        .arg("encrypt")
        .arg(&first)
        .args(["--user-password", "a", "--owner-password", "b", "-o"])
        .arg(&second)
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "encrypting an encrypted input succeeded"
    );
    assert!(!second.exists(), "an output file was written");
}
