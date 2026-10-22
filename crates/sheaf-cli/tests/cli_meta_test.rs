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
fn test_cli_meta_get_on_standard_fixtures_and_encrypted_files() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let f12 = fixtures.join("fixture_12p.pdf");
    let f_a = fixtures.join("fixture_a_2p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    // 1. Single file meta get with JSON envelope
    let out_single = sheaf_cmd()
        .args(["meta", "get", f12.to_str().unwrap(), "--json"])
        .output()
        .expect("run meta get on fixture_12p.pdf");

    assert_eq!(out_single.status.code(), Some(0));
    let json_single: Value = serde_json::from_slice(&out_single.stdout).expect("parse json");
    assert_eq!(json_single["ok"], true);
    assert_eq!(json_single["op"], "meta get");

    let files = json_single["data"]["files"]
        .as_array()
        .expect("files array");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["info"]["title"], "Report12");
    assert_eq!(files[0]["info"]["author"], "Sheaf Test");

    // 2. Encrypted file behavior
    let enc_pdf = temp.path().join("meta_locked.pdf");
    let prot_out = sheaf_cmd()
        .args([
            "protect",
            f_a.to_str().unwrap(),
            "--new-password-stdin",
            "-o",
            enc_pdf.to_str().unwrap(),
        ])
        .write_stdin("meta-pass-123\n")
        .output()
        .expect("protect file");
    assert_eq!(prot_out.status.code(), Some(0));

    // Calling meta get without password must exit with code 4 ("encrypted")
    let out_enc_no_pw = sheaf_cmd()
        .args(["meta", "get", enc_pdf.to_str().unwrap(), "--json"])
        .output()
        .expect("meta get on encrypted without pw");

    assert_eq!(out_enc_no_pw.status.code(), Some(4));
    let json_enc_no_pw: Value = serde_json::from_slice(&out_enc_no_pw.stdout).expect("parse json");
    assert_eq!(json_enc_no_pw["ok"], false);
    assert_eq!(json_enc_no_pw["error"]["code"], "encrypted");

    // Calling meta get with correct password succeeds (Exit 0)
    let out_enc_with_pw = sheaf_cmd()
        .args([
            "meta",
            "get",
            enc_pdf.to_str().unwrap(),
            "--password-stdin",
            "--json",
        ])
        .write_stdin("meta-pass-123\n")
        .output()
        .expect("meta get on encrypted with pw");

    assert_eq!(out_enc_with_pw.status.code(), Some(0));
    let json_enc_with_pw: Value =
        serde_json::from_slice(&out_enc_with_pw.stdout).expect("parse json");
    assert_eq!(json_enc_with_pw["ok"], true);

    // 3. Batch meta get across multiple files
    let out_batch = sheaf_cmd()
        .args([
            "meta",
            "get",
            f12.to_str().unwrap(),
            f_a.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("meta get batch");

    assert_eq!(out_batch.status.code(), Some(0));
    let json_batch: Value = serde_json::from_slice(&out_batch.stdout).expect("parse json");
    assert_eq!(json_batch["data"]["files"].as_array().unwrap().len(), 2);
}
