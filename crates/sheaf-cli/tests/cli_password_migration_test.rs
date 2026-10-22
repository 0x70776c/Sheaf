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
fn test_protect_and_unlock_reject_legacy_password_flags_with_exit_2() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_a_2p.pdf");

    // 1. sheaf protect --password
    let out_pw = sheaf_cmd()
        .args([
            "protect",
            src.to_str().unwrap(),
            "--password",
            "legacy-pass",
        ])
        .output()
        .expect("run protect --password");

    assert_eq!(out_pw.status.code(), Some(2));
    let stderr_pw = String::from_utf8_lossy(&out_pw.stderr);
    assert!(
        stderr_pw.contains("--password wurde entfernt (Sicherheitsrisiko). Nutze --password-env VAR, --password-file PFAD oder --password-stdin."),
        "Unexpected stderr: {stderr_pw}"
    );

    // 2. sheaf protect --owner-password
    let out_owner = sheaf_cmd()
        .args([
            "protect",
            src.to_str().unwrap(),
            "--owner-password",
            "legacy-owner",
        ])
        .output()
        .expect("run protect --owner-password");

    assert_eq!(out_owner.status.code(), Some(2));
    let stderr_owner = String::from_utf8_lossy(&out_owner.stderr);
    assert!(
        stderr_owner.contains("--owner-password wurde entfernt (Sicherheitsrisiko). Nutze --new-owner-password-env VAR oder --new-owner-password-file PFAD."),
        "Unexpected stderr: {stderr_owner}"
    );

    // 3. sheaf unlock --password
    let out_unlock = sheaf_cmd()
        .args(["unlock", src.to_str().unwrap(), "--password", "legacy-pass"])
        .output()
        .expect("run unlock --password");

    assert_eq!(out_unlock.status.code(), Some(2));
    let stderr_unlock = String::from_utf8_lossy(&out_unlock.stderr);
    assert!(
        stderr_unlock.contains("--password wurde entfernt (Sicherheitsrisiko). Nutze --password-env VAR, --password-file PFAD oder --password-stdin."),
        "Unexpected stderr: {stderr_unlock}"
    );

    // 4. sheaf protect --password --json
    let out_json = sheaf_cmd()
        .args([
            "protect",
            src.to_str().unwrap(),
            "--password",
            "legacy-pass",
            "--json",
        ])
        .output()
        .expect("run protect --password --json");

    assert_eq!(out_json.status.code(), Some(2));
    let env: Value = serde_json::from_slice(&out_json.stdout).expect("parse json");
    assert_eq!(env["ok"], false);
    assert_eq!(env["error"]["exit_code"], 2);
    assert!(
        env["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--password wurde entfernt"),
        "Unexpected json error: {env:?}"
    );
}
