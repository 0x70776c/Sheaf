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

    // 2. sheaf protect with --new-password-env and --new-owner-password-env
    let protected_pdf = temp.path().join("protected.pdf");
    let out_prot = sheaf_cmd()
        .env("SHEAF_TEST_USER_PW", "cli-secret")
        .env("SHEAF_TEST_OWNER_PW", "cli-owner")
        .args([
            "protect",
            wm_pdf.to_str().unwrap(),
            "--new-password-env",
            "SHEAF_TEST_USER_PW",
            "--new-owner-password-env",
            "SHEAF_TEST_OWNER_PW",
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
    assert_eq!(env_prot["data"]["algorithm"], "aes-256");
    assert_eq!(env_prot["data"]["key_bits"], 256);
    assert_eq!(env_prot["data"]["permissions"]["copy"], false);
    assert_eq!(env_prot["data"]["permissions"]["print"], true);
    assert_eq!(env_prot["data"]["permissions"]["modify"], true);

    // 3. sheaf unlock with wrong password -> exit code 4 (wrong_password)
    let bad_unlock = temp.path().join("bad_unlock.pdf");
    let out_bad = sheaf_cmd()
        .env("SHEAF_TEST_BAD_PW", "wrong-pw")
        .args([
            "unlock",
            protected_pdf.to_str().unwrap(),
            "--password-env",
            "SHEAF_TEST_BAD_PW",
            "-o",
            bad_unlock.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run sheaf unlock");
    assert_eq!(out_bad.status.code(), Some(4));
    let env_bad: Value = serde_json::from_slice(&out_bad.stdout).expect("parse json");
    assert_eq!(env_bad["ok"], false);
    assert_eq!(env_bad["error"]["code"], "wrong_password");

    // 4. sheaf unlock with correct password -> exit code 0
    let unlocked_pdf = temp.path().join("unlocked.pdf");
    let out_unl = sheaf_cmd()
        .env("SHEAF_TEST_GOOD_PW", "cli-secret")
        .args([
            "unlock",
            protected_pdf.to_str().unwrap(),
            "--password-env",
            "SHEAF_TEST_GOOD_PW",
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
    assert!(
        env_unl["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("Eingabe war verschlüsselt")),
        "expected warning that encrypted input yielded unencrypted output"
    );

    // 4b. sheaf unlock on unencrypted PDF succeeds with Exit 0 and warning "Datei ist nicht verschlüsselt"
    let unenc_copy = temp.path().join("unenc_copy.pdf");
    let out_plain_unl = sheaf_cmd()
        .env("SHEAF_TEST_GOOD_PW", "cli-secret")
        .args([
            "unlock",
            src_3p.to_str().unwrap(),
            "--password-env",
            "SHEAF_TEST_GOOD_PW",
            "-o",
            unenc_copy.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run sheaf unlock on unencrypted pdf");
    assert_eq!(out_plain_unl.status.code(), Some(0));
    assert!(unenc_copy.exists());
    let env_plain_unl: Value = serde_json::from_slice(&out_plain_unl.stdout).expect("parse json");
    assert_eq!(env_plain_unl["ok"], true);
    assert!(
        env_plain_unl["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w
                .as_str()
                .unwrap()
                .contains("Datei ist nicht verschlüsselt")),
        "expected warning that file is not encrypted"
    );

    // 4c. sheaf verify without password on encrypted PDF fails with Exit 4 (encrypted)
    let out_ver = sheaf_cmd()
        .args(["verify", protected_pdf.to_str().unwrap(), "--json"])
        .output()
        .expect("run verify on protected pdf");
    assert_eq!(out_ver.status.code(), Some(4));
    let env_ver: Value = serde_json::from_slice(&out_ver.stdout).expect("parse json");
    assert_eq!(env_ver["ok"], false);
    assert_eq!(env_ver["error"]["code"], "encrypted");

    // 5. sheaf number
    let numbered_pdf = temp.path().join("numbered.pdf");
    let out_num = sheaf_cmd()
        .args([
            "number",
            src_3p.to_str().unwrap(),
            "--format",
            "Page {n} of {total} ({page}/{pages})",
            "--position",
            "bottom-right",
            "--start",
            "1",
            "--pages",
            "2-3",
            "-o",
            numbered_pdf.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run sheaf number");
    assert_eq!(out_num.status.code(), Some(0));
    assert!(numbered_pdf.exists());
    let env_num: Value = serde_json::from_slice(&out_num.stdout).expect("parse json");
    assert_eq!(env_num["ok"], true);
    assert_eq!(env_num["op"], "number");
    assert_eq!(env_num["data"]["position"], "bottom-right");
    assert_eq!(env_num["data"]["pages_numbered"], serde_json::json!([2, 3]));
}

#[test]
fn test_protect_help_contains_permission_hint_and_protect_without_password_fails_exit_2() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_a_2p.pdf");

    // 1. protect --help contains permission disclaimer
    let help_out = sheaf_cmd()
        .args(["protect", "--help"])
        .output()
        .expect("run sheaf protect --help");
    assert_eq!(help_out.status.code(), Some(0));
    let help_text = String::from_utf8_lossy(&help_out.stdout);
    assert!(
        help_text.contains("--no-print, --no-copy, --no-modify sind Hinweise an den PDF-Viewer. Manche Programme ignorieren sie. Echter Schutz entsteht nur durch das User-Passwort."),
        "Missing exact permission notice in protect --help:\n{help_text}"
    );

    // 2. Calling protect without any --new-password-* flags fails with exit 2
    let out_no_pw = sheaf_cmd()
        .args(["protect", src.to_str().unwrap(), "--json"])
        .output()
        .expect("run sheaf protect without password");
    assert_eq!(out_no_pw.status.code(), Some(2));
    let json_no_pw: Value = serde_json::from_slice(&out_no_pw.stdout).expect("parse json");
    assert_eq!(json_no_pw["ok"], false);
    assert_eq!(json_no_pw["error"]["exit_code"], 2);
    assert!(
        json_no_pw["error"]["message"].as_str().unwrap().contains("Missing required password. Specify --new-password-env VAR, --new-password-file PATH, or --new-password-stdin."),
        "Unexpected error message: {}",
        json_no_pw["error"]["message"]
    );
}
