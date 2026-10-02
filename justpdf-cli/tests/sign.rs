//! Integration tests for `justpdf sign`.

use std::path::Path;
use std::process::Command;

#[test]
fn sign_fails_without_writing_output() {
    let input = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/compressible.pdf");
    let output = std::env::temp_dir().join("justpdf_cli_sign_out.pdf");
    let _ = std::fs::remove_file(&output);

    let result = Command::new(env!("CARGO_BIN_EXE_justpdf"))
        .arg("sign")
        .arg(&input)
        .args(["--cert", "unused.p12", "-o"])
        .arg(&output)
        .output()
        .unwrap();

    assert!(!result.status.success(), "sign exited with success");
    assert_eq!(result.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("not implemented"), "stderr: {stderr}");
    assert!(!output.exists());
}
