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
fn cli_watermark_protect_and_unlock_end_to_end() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src_3p = fixtures.join("fixture_b_3p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    // 1. sheaf watermark
    let wm_pdf = temp.path().join("watermarked.pdf");
    let out_wm = sheaf_cmd()
        .args([
            "watermark",
            src_3p.to_str().unwrap(),
            "--text",
            "CONFIDENTIAL",
            "--position",
            "diagonal",
            "--opacity",
            "0.3",
            "--font-size",
            "42",
            "--color",
            "red",
            "--pages",
            "1-2",
            "-o",
            wm_pdf.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run sheaf watermark");
    assert_eq!(out_wm.status.code(), Some(0));
    assert!(wm_pdf.exists());
    let env_wm: Value = serde_json::from_slice(&out_wm.stdout).expect("parse json");
    assert_eq!(env_wm["ok"], true);
    assert_eq!(env_wm["op"], "watermark");
    assert_eq!(env_wm["data"]["color"], "#DC2626");
    assert_eq!(
        env_wm["data"]["pages_watermarked"],
        serde_json::json!([1, 2])
    );

    // 2. sheaf protect
    let protected_pdf = temp.path().join("protected.pdf");
    let out_prot = sheaf_cmd()
        .args([
            "protect",
            wm_pdf.to_str().unwrap(),
            "--password",
            "cli-secret",
            "--owner-password",
            "cli-owner",
            "--no-copy",
            "-o",
            protected_pdf.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run sheaf protect");
    assert_eq!(out_prot.status.code(), Some(0));
    assert!(protected_pdf.exists());
    let env_prot: Value = serde_json::from_slice(&out_prot.stdout).expect("parse json");
    assert_eq!(env_prot["ok"], true);
    assert_eq!(env_prot["op"], "protect");
    assert_eq!(env_prot["data"]["encrypted"], true);
    assert_eq!(env_prot["data"]["pages"], 3);

    // 3. sheaf unlock with wrong password -> exit code 4 (wrong_password)
    let bad_unlock = temp.path().join("bad_unlock.pdf");
    let out_bad = sheaf_cmd()
        .args([
            "unlock",
            protected_pdf.to_str().unwrap(),
            "--password",
            "wrong-pw",
            "-o",
            bad_unlock.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run sheaf unlock wrong pw");
    assert_eq!(out_bad.status.code(), Some(4));
    let env_bad: Value = serde_json::from_slice(&out_bad.stdout).expect("parse json");
    assert_eq!(env_bad["error"]["code"], "wrong_password");

    // 4. sheaf unlock with correct password -> exit code 0
    let unlocked_pdf = temp.path().join("unlocked.pdf");
    let out_unl = sheaf_cmd()
        .args([
            "unlock",
            protected_pdf.to_str().unwrap(),
            "--password",
            "cli-secret",
            "-o",
            unlocked_pdf.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run sheaf unlock correct pw");
    assert_eq!(out_unl.status.code(), Some(0));
    assert!(unlocked_pdf.exists());
    let env_unl: Value = serde_json::from_slice(&out_unl.stdout).expect("parse json");
    assert_eq!(env_unl["ok"], true);
    assert_eq!(env_unl["op"], "unlock");
    assert_eq!(env_unl["data"]["encrypted"], false);
    assert_eq!(env_unl["data"]["pages"], 3);
}
