//! Integration tests for the `justpdf compress` subcommand.
//!
//! Drives the built binary end-to-end against a fixture that is known to
//! shrink under every preset, asserting the output is a smaller, valid PDF.

use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_justpdf"))
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/compressible.pdf")
}

/// A valid PDF starts with the `%PDF-` header and ends with `%%EOF`.
fn is_valid_pdf(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF-") && bytes.windows(5).any(|w| w == b"%%EOF")
}

#[test]
fn compresses_with_default_medium_preset() {
    let out = std::env::temp_dir().join("justpdf_cli_compress_default.pdf");
    let _ = std::fs::remove_file(&out);

    let status = bin()
        .arg("compress")
        .arg(fixture())
        .arg("-o")
        .arg(&out)
        .status()
        .unwrap();
    assert!(status.success(), "compress should exit successfully");

    let orig = std::fs::metadata(fixture()).unwrap().len();
    let result = std::fs::read(&out).unwrap();
    assert!(is_valid_pdf(&result), "output should be a valid PDF");
    assert!(
        (result.len() as u64) <= orig,
        "compressed ({}) should be <= original ({})",
        result.len(),
        orig
    );
}

#[test]
fn every_preset_produces_a_valid_smaller_pdf() {
    let orig = std::fs::metadata(fixture()).unwrap().len();
    for preset in ["low", "medium", "high", "extreme"] {
        let out = std::env::temp_dir().join(format!("justpdf_cli_compress_{preset}.pdf"));
        let _ = std::fs::remove_file(&out);

        let status = bin()
            .arg("compress")
            .arg(fixture())
            .arg("--preset")
            .arg(preset)
            .arg("-o")
            .arg(&out)
            .status()
            .unwrap();
        assert!(status.success(), "preset {preset} should succeed");

        let result = std::fs::read(&out).unwrap();
        assert!(
            is_valid_pdf(&result),
            "preset {preset} output should be valid PDF"
        );
        assert!(
            (result.len() as u64) <= orig,
            "preset {preset}: compressed ({}) should be <= original ({})",
            result.len(),
            orig
        );
    }
}

#[test]
fn unknown_preset_is_rejected() {
    let out = std::env::temp_dir().join("justpdf_cli_compress_bogus.pdf");
    let _ = std::fs::remove_file(&out);

    let output = bin()
        .arg("compress")
        .arg(fixture())
        .arg("--preset")
        .arg("bogus")
        .arg("-o")
        .arg(&out)
        .output()
        .unwrap();

    assert!(!output.status.success(), "unknown preset should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Unknown preset"),
        "stderr should explain the bad preset, got: {stderr}"
    );
    assert!(!out.exists(), "no output file should be written on error");
}

#[test]
fn reports_reduction_on_stderr_not_stdout() {
    let out = std::env::temp_dir().join("justpdf_cli_compress_report.pdf");
    let _ = std::fs::remove_file(&out);

    let output = bin()
        .arg("compress")
        .arg(fixture())
        .arg("-o")
        .arg(&out)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.is_empty(), "stdout should stay clean, got: {stdout}");
    assert!(
        stderr.contains("Compressed:") && stderr.contains("reduction"),
        "stderr should carry the one-line report, got: {stderr}"
    );
}

#[test]
fn missing_output_flag_is_an_error() {
    let output = bin().arg("compress").arg(fixture()).output().unwrap();
    assert!(!output.status.success(), "missing -o should fail");
}

#[test]
fn conflicting_on_off_pair_is_rejected() {
    let out = std::env::temp_dir().join("justpdf_cli_compress_conflict.pdf");
    let _ = std::fs::remove_file(&out);
    let output = bin()
        .arg("compress")
        .arg(fixture())
        .arg("-o")
        .arg(&out)
        .arg("--grayscale")
        .arg("--no-grayscale")
        .output()
        .unwrap();
    assert!(!output.status.success(), "--x with --no-x should fail");
    assert!(!out.exists(), "no output on a conflicting invocation");
}

#[test]
fn jpeg_quality_conflicts_with_no_image_recompress() {
    let out = std::env::temp_dir().join("justpdf_cli_compress_jpeg_conflict.pdf");
    let _ = std::fs::remove_file(&out);
    let output = bin()
        .arg("compress")
        .arg(fixture())
        .arg("-o")
        .arg(&out)
        .arg("--jpeg-quality")
        .arg("80")
        .arg("--no-image-recompress")
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "quality + disable should conflict"
    );
}

#[test]
fn jpeg_quality_out_of_range_is_rejected() {
    let out = std::env::temp_dir().join("justpdf_cli_compress_range.pdf");
    let _ = std::fs::remove_file(&out);
    for bad in ["0", "101"] {
        let output = bin()
            .arg("compress")
            .arg(fixture())
            .arg("-o")
            .arg(&out)
            .arg("--jpeg-quality")
            .arg(bad)
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "jpeg-quality {bad} should be rejected"
        );
    }
}

#[test]
fn overrides_layer_onto_preset_and_still_compress() {
    let out = std::env::temp_dir().join("justpdf_cli_compress_override.pdf");
    let _ = std::fs::remove_file(&out);
    let status = bin()
        .arg("compress")
        .arg(fixture())
        .arg("--preset")
        .arg("high")
        .arg("--no-strip-metadata")
        .arg("--jpeg-quality")
        .arg("80")
        .arg("-o")
        .arg(&out)
        .status()
        .unwrap();
    assert!(status.success(), "high + overrides should succeed");
    let result = std::fs::read(&out).unwrap();
    assert!(
        is_valid_pdf(&result),
        "override output should be a valid PDF"
    );
}

fn image_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/with_image.pdf")
}

#[test]
fn analyze_reports_pages_and_images_on_stdout() {
    let output = bin()
        .arg("compress")
        .arg(image_fixture())
        .arg("--analyze")
        .output()
        .unwrap();

    assert!(output.status.success(), "analyze should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Pages: 1"),
        "expected 1 page, got: {stdout}"
    );
    assert!(
        stdout.contains("Images: 1"),
        "expected 1 image, got: {stdout}"
    );
    assert!(
        stdout.contains("Encrypted: No"),
        "expected encrypted status, got: {stdout}"
    );
    // Image data is shown human-readably (KB/MB).
    assert!(
        stdout.contains("Image data:") && stdout.contains("KB"),
        "got: {stdout}"
    );
}

#[test]
fn analyze_needs_no_output_flag_and_writes_nothing() {
    // No -o is given; analyze must neither require it nor write any file.
    let status = bin()
        .arg("compress")
        .arg(image_fixture())
        .arg("--analyze")
        .status()
        .unwrap();
    assert!(status.success(), "analyze without -o should succeed");

    // No output appeared next to the input.
    let sibling = image_fixture().with_file_name("with_image.out.pdf");
    assert!(!sibling.exists());
}

#[test]
fn verbose_prints_breakdown_on_stderr() {
    let out = std::env::temp_dir().join("justpdf_cli_verbose_out.pdf");
    let _ = std::fs::remove_file(&out);

    let output = bin()
        .arg("compress")
        .arg(image_fixture())
        .arg("--preset")
        .arg("high")
        .arg("--verbose")
        .arg("-o")
        .arg(&out)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.is_empty(), "stdout must stay clean, got: {stdout}");
    assert!(stderr.contains("Compressed:"), "summary missing: {stderr}");
    assert!(
        stderr.contains("Images found: 1"),
        "verbose breakdown should report the image, got: {stderr}"
    );
}

#[test]
fn non_verbose_omits_breakdown() {
    let out = std::env::temp_dir().join("justpdf_cli_nonverbose_out.pdf");
    let _ = std::fs::remove_file(&out);

    let output = bin()
        .arg("compress")
        .arg(image_fixture())
        .arg("--preset")
        .arg("high")
        .arg("-o")
        .arg(&out)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Compressed:"), "summary missing: {stderr}");
    assert!(
        !stderr.contains("Images found:"),
        "no breakdown without --verbose, got: {stderr}"
    );
}
