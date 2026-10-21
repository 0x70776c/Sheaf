use std::path::{Path, PathBuf};

use assert_cmd::Command;
use serde_json::Value;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn sheaf_cmd() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("sheaf"))
}

#[test]
fn cli_compress_end_to_end_and_error_codes() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let photo_pdf = fixtures.join("fixture_photo.pdf");
    let encrypted = fixtures.join("encrypted.pdf");
    let corrupt = fixtures.join("corrupted.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    // 1. Compress photo PDF with --preset medium --json
    let out_med = temp.path().join("photo_med.pdf");
    let out = sheaf_cmd()
        .args([
            "compress",
            photo_pdf.to_str().unwrap(),
            "--preset",
            "medium",
            "-o",
            out_med.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run sheaf compress");
    assert_eq!(out.status.code(), Some(0));
    assert!(out_med.exists());

    let env: Value = serde_json::from_slice(&out.stdout).expect("parse json");
    assert_eq!(env["schema"], 1);
    assert_eq!(env["ok"], true);
    assert_eq!(env["op"], "compress");
    assert_eq!(env["data"]["preset"], "medium");
    assert!(env["data"]["saved_percent"].as_f64().unwrap() > 40.0);
    assert!(env["data"]["images_resampled"].as_u64().unwrap() >= 1);

    // 2. OutputExists without --force (exit code 7)
    let out_exists = sheaf_cmd()
        .args([
            "compress",
            photo_pdf.to_str().unwrap(),
            "--preset",
            "medium",
            "-o",
            out_med.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run sheaf compress existing");
    assert_eq!(out_exists.status.code(), Some(7));
    let err_env: Value = serde_json::from_slice(&out_exists.stdout).expect("parse json");
    assert_eq!(err_env["error"]["code"], "output_exists");

    // 3. Compress with --preset high --grayscale --dry-run
    let dry_out = temp.path().join("dry_high.pdf");
    let out_dry = sheaf_cmd()
        .args([
            "compress",
            photo_pdf.to_str().unwrap(),
            "--preset",
            "high",
            "--grayscale",
            "--dry-run",
            "-o",
            dry_out.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run sheaf compress dry-run");
    assert_eq!(out_dry.status.code(), Some(0));
    assert!(!dry_out.exists());
    let dry_env: Value = serde_json::from_slice(&out_dry.stdout).expect("parse json");
    assert_eq!(dry_env["data"]["grayscale"], true);
    assert_eq!(dry_env["data"]["dry_run"], true);

    // 4. Encrypted (exit code 4)
    let out_enc = sheaf_cmd()
        .args([
            "compress",
            encrypted.to_str().unwrap(),
            "-o",
            temp.path().join("enc.pdf").to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run sheaf compress encrypted");
    assert_eq!(out_enc.status.code(), Some(4));

    // 5. InvalidPdf (exit code 3)
    let out_cor = sheaf_cmd()
        .args([
            "compress",
            corrupt.to_str().unwrap(),
            "-o",
            temp.path().join("cor.pdf").to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run sheaf compress corrupt");
    assert_eq!(out_cor.status.code(), Some(3));
}
