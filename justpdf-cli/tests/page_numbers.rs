//! Integration tests for 1-based page options: `text --page`, `render --page`, `split --pages`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A one-page fixture.
fn one_page_pdf() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/compressible.pdf")
}

fn justpdf(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_justpdf"))
        .args(args)
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Exited with an error code, not a panic (101).
fn assert_rejected(output: &Output) {
    let code = output.status.code();
    assert!(
        matches!(code, Some(1) | Some(2)),
        "exit {code:?}, stderr: {}",
        stderr(output)
    );
    assert!(!stderr(output).contains("panicked"), "{}", stderr(output));
}

#[test]
fn text_page_zero_is_rejected() {
    let pdf = one_page_pdf();
    let out = justpdf(&[
        "text".as_ref(),
        pdf.as_os_str(),
        "--page".as_ref(),
        "0".as_ref(),
    ]);
    assert_rejected(&out);
    assert!(stderr(&out).contains("--page"), "{}", stderr(&out));
}

#[test]
fn render_page_zero_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = one_page_pdf();
    let png = dir.path().join("page.png");
    let out = justpdf(&[
        "render".as_ref(),
        pdf.as_os_str(),
        "--page".as_ref(),
        "0".as_ref(),
        "-o".as_ref(),
        png.as_os_str(),
    ]);
    assert_rejected(&out);
    assert!(stderr(&out).contains("--page"), "{}", stderr(&out));
    assert!(!png.exists());
}

#[test]
fn text_page_past_the_end_reports_one_based_page() {
    let pdf = one_page_pdf();
    let out = justpdf(&[
        "text".as_ref(),
        pdf.as_os_str(),
        "--page".as_ref(),
        "99".as_ref(),
    ]);
    assert_rejected(&out);
    assert!(
        stderr(&out).contains("Page 99 out of range (total: 1)"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn text_first_page_still_works() {
    let pdf = one_page_pdf();
    let out = justpdf(&[
        "text".as_ref(),
        pdf.as_os_str(),
        "--page".as_ref(),
        "1".as_ref(),
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
}

#[test]
fn split_range_starting_at_zero_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = one_page_pdf();
    let output = dir.path().join("out.pdf");
    let out = justpdf(&[
        "split".as_ref(),
        pdf.as_os_str(),
        "--pages".as_ref(),
        "0-1".as_ref(),
        "-o".as_ref(),
        output.as_os_str(),
    ]);
    assert_rejected(&out);
    assert!(
        stderr(&out).contains("Page 0 out of range (1-1)"),
        "{}",
        stderr(&out)
    );
    assert!(!output.exists());
}
