//! Integration tests for `justpdf compress` on encrypted input.
//!
//! The fixture is built with justpdf-core's DocumentBuilder (a non-empty user
//! password, so authentication is genuinely required) and written to a temp
//! file, then driven through the binary end-to-end.

use std::path::{Path, PathBuf};
use std::process::Command;

use justpdf_core::crypto;
use justpdf_core::writer::document::DocumentBuilder;
use justpdf_core::writer::page::PageBuilder;

// A password pair that round-trips cleanly through core's security handler.
const USER_PW: &str = "user123";
const OWNER_PW: &str = "owner456";

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_justpdf"))
}

/// Build a small AES-128 encrypted PDF requiring `USER_PW`, write it into
/// `dir`, and return its path.
fn encrypted_fixture(dir: &Path) -> PathBuf {
    let mut builder = DocumentBuilder::new();
    let font = builder.add_standard_font("Helvetica");
    let mut page = PageBuilder::new(612.0, 792.0);
    page.add_font(&font, "Helvetica");
    page.begin_text();
    page.set_font(&font, 24.0);
    page.move_to(72.0, 720.0);
    page.show_text("Encrypted content");
    page.end_text();
    builder.add_page(page);
    builder.set_encryption(crypto::EncryptionConfig {
        user_password: USER_PW.as_bytes().to_vec(),
        owner_password: OWNER_PW.as_bytes().to_vec(),
        permissions: crypto::Permissions::allow_all(),
        method: crypto::EncryptionMethod::AES128,
        encrypt_metadata: true,
    });
    let bytes = builder.build().unwrap();

    let path = dir.join("in.pdf");
    std::fs::write(&path, &bytes).unwrap();
    path
}

fn is_valid_pdf(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF-") && bytes.windows(5).any(|w| w == b"%%EOF")
}

fn is_encrypted(bytes: &[u8]) -> bool {
    justpdf_core::PdfDocument::from_bytes(bytes.to_vec())
        .map(|d| d.is_encrypted())
        .unwrap_or(false)
}

#[test]
fn compresses_encrypted_pdf_with_password_and_drops_encryption() {
    let dir = tempfile::tempdir().unwrap();
    let input = encrypted_fixture(dir.path());
    assert!(
        is_encrypted(&std::fs::read(&input).unwrap()),
        "fixture must be encrypted"
    );

    let out = dir.path().join("out.pdf");

    let output = bin()
        .arg("compress")
        .arg(&input)
        .arg("-o")
        .arg(&out)
        .arg("--password")
        .arg(USER_PW)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "compress with password should succeed"
    );
    let result = std::fs::read(&out).unwrap();
    assert!(is_valid_pdf(&result), "output should be a valid PDF");
    assert!(!is_encrypted(&result), "output must be decrypted");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("encryption removed"),
        "stderr should warn that encryption was removed, got: {stderr}"
    );
}

#[test]
fn encrypted_pdf_without_password_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let input = encrypted_fixture(dir.path());
    let out = dir.path().join("out.pdf");

    let output = bin()
        .arg("compress")
        .arg(&input)
        .arg("-o")
        .arg(&out)
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "encrypted input without --password should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.to_lowercase().contains("encrypt"),
        "error should mention encryption, got: {stderr}"
    );
    assert!(!out.exists(), "no output should be written");
}

#[test]
fn wrong_password_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let input = encrypted_fixture(dir.path());
    let out = dir.path().join("out.pdf");

    let output = bin()
        .arg("compress")
        .arg(&input)
        .arg("-o")
        .arg(&out)
        .arg("--password")
        .arg("definitely-not-it")
        .output()
        .unwrap();

    assert!(!output.status.success(), "wrong password should fail");
    assert!(!out.exists(), "no output should be written");
}
