//! Integration tests for `justpdf convert` with MOBI and FB2 input.

use std::path::Path;
use std::process::{Command, Output};

const FB2: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0">
  <description>
    <title-info>
      <book-title>Convert Test</book-title>
    </title-info>
  </description>
  <body>
    <section>
      <p>Hello FB2</p>
    </section>
  </body>
</FictionBook>"#;

/// A minimal PDB/MOBI file holding `text` as one uncompressed PalmDOC record.
fn mobi(text: &str) -> Vec<u8> {
    let mut data = Vec::new();
    let mut name = [0u8; 32];
    name[..9].copy_from_slice(b"Test Book");
    data.extend_from_slice(&name);
    data.extend_from_slice(&[0u8; 28]); // attributes .. sort info offset
    data.extend_from_slice(b"BOOKMOBI");
    data.extend_from_slice(&[0u8; 8]); // unique ID seed, next record list
    data.extend_from_slice(&2u16.to_be_bytes()); // record count

    let rec0: u32 = 78 + 16;
    let rec1: u32 = rec0 + 16;
    data.extend_from_slice(&rec0.to_be_bytes());
    data.extend_from_slice(&[0u8; 4]);
    data.extend_from_slice(&rec1.to_be_bytes());
    data.extend_from_slice(&[0u8; 4]);

    // Record 0: PalmDOC header, compression 1 (none).
    data.extend_from_slice(&1u16.to_be_bytes());
    data.extend_from_slice(&[0u8; 2]);
    data.extend_from_slice(&(text.len() as u32).to_be_bytes());
    data.extend_from_slice(&1u16.to_be_bytes());
    data.extend_from_slice(&4096u16.to_be_bytes());
    data.extend_from_slice(&[0u8; 4]);

    // Record 1: the text.
    data.extend_from_slice(text.as_bytes());
    data
}

fn convert(input: &Path, output: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_justpdf"))
        .arg("convert")
        .arg(input)
        .arg("-o")
        .arg(output)
        .output()
        .unwrap()
}

fn assert_pdf_with_pages(path: &Path) {
    let doc = justpdf_core::PdfDocument::open(path).unwrap();
    let pages = justpdf_core::page::page_count(&doc).unwrap();
    assert!(pages >= 1, "no pages in {}", path.display());
}

#[test]
fn fb2_converts_to_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("book.fb2");
    let output = dir.path().join("book.pdf");
    std::fs::write(&input, FB2).unwrap();

    let result = convert(&input, &output);

    assert!(
        result.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_pdf_with_pages(&output);
}

#[test]
fn mobi_converts_to_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("book.pdf");
    for ext in ["mobi", "prc"] {
        let input = dir.path().join(format!("book.{ext}"));
        std::fs::write(&input, mobi("Hello MOBI")).unwrap();

        let result = convert(&input, &output);

        assert!(
            result.status.success(),
            ".{ext} stderr: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_pdf_with_pages(&output);
        std::fs::remove_file(&output).unwrap();
    }
}

#[test]
fn fb2_and_mobi_convert_to_png() {
    let dir = tempfile::tempdir().unwrap();
    let fb2 = dir.path().join("book.fb2");
    let mobi_path = dir.path().join("book.mobi");
    std::fs::write(&fb2, FB2).unwrap();
    std::fs::write(&mobi_path, mobi("Hello MOBI")).unwrap();

    for input in [&fb2, &mobi_path] {
        let output = dir.path().join("page.png");
        let result = convert(input, &output);

        assert!(
            result.status.success(),
            "{} stderr: {}",
            input.display(),
            String::from_utf8_lossy(&result.stderr)
        );
        let png = std::fs::read(&output).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "{}", input.display());
        std::fs::remove_file(&output).unwrap();
    }
}

#[test]
fn unsupported_output_fails_without_writing() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("book.fb2");
    let output = dir.path().join("book.svg");
    std::fs::write(&input, FB2).unwrap();

    let result = convert(&input, &output);

    assert!(
        !result.status.success(),
        "convert to svg exited with success"
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("unsupported output format: svg"),
        "stderr: {stderr}"
    );
    assert!(!output.exists());
}
