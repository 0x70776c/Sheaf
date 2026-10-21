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
fn cli_render_end_to_end_and_error_codes() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src_3p = fixtures.join("fixture_b_3p.pdf");
    let encrypted = fixtures.join("encrypted.pdf");
    let corrupt = fixtures.join("corrupted.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    // 1. Render single page by --width to explicit .png file
    let page1_png = temp.path().join("cover.png");
    let out = sheaf_cmd()
        .args([
            "render",
            src_3p.to_str().unwrap(),
            "--page",
            "1",
            "--width",
            "160",
            "-o",
            page1_png.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run sheaf render");
    assert_eq!(out.status.code(), Some(0));
    assert!(page1_png.exists());

    let env: Value = serde_json::from_slice(&out.stdout).expect("parse json");
    assert_eq!(env["schema"], 1);
    assert_eq!(env["ok"], true);
    assert_eq!(env["op"], "render");
    assert_eq!(env["data"]["total_pages"], 3);
    assert_eq!(env["data"]["outputs"][0]["page"], 1);
    assert_eq!(env["data"]["outputs"][0]["width_px"], 160);

    // 2. OutputExists without --force (exit code 7)
    let out_exists = sheaf_cmd()
        .args([
            "render",
            src_3p.to_str().unwrap(),
            "--page",
            "1",
            "--width",
            "160",
            "-o",
            page1_png.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run sheaf render existing");
    assert_eq!(out_exists.status.code(), Some(7));
    let err_env: Value = serde_json::from_slice(&out_exists.stdout).expect("parse json");
    assert_eq!(err_env["error"]["code"], "output_exists");

    // 3. Render multiple pages with --dpi and --dry-run
    let dry_dir = temp.path().join("dry_thumbs");
    let out_dry = sheaf_cmd()
        .args([
            "render",
            src_3p.to_str().unwrap(),
            "--pages",
            "1-2",
            "--dpi",
            "150",
            "--dry-run",
            "-o",
            dry_dir.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run sheaf render dry-run");
    assert_eq!(out_dry.status.code(), Some(0));
    assert!(!dry_dir.exists());

    // 4. PageOutOfRange (exit code 5)
    let out_oob = sheaf_cmd()
        .args([
            "render",
            src_3p.to_str().unwrap(),
            "--page",
            "99",
            "-o",
            temp.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run sheaf render oob");
    assert_eq!(out_oob.status.code(), Some(5));

    // 5. Encrypted (exit code 4)
    let out_enc = sheaf_cmd()
        .args([
            "render",
            encrypted.to_str().unwrap(),
            "--page",
            "1",
            "-o",
            temp.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run sheaf render encrypted");
    assert_eq!(out_enc.status.code(), Some(4));

    // 6. InvalidPdf (exit code 3)
    let out_corrupt = sheaf_cmd()
        .args([
            "render",
            corrupt.to_str().unwrap(),
            "--page",
            "1",
            "-o",
            temp.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run sheaf render corrupt");
    assert_eq!(out_corrupt.status.code(), Some(3));
}
