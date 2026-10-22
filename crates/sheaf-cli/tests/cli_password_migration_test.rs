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
fn test_migration_legacy_password_rejected() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_a_2p.pdf");

    // 1. protect --password
    let out_prot = sheaf_cmd()
        .args([
            "protect",
            src.to_str().unwrap(),
            "--password",
            "legacy-pass",
            "--json",
        ])
        .output()
        .expect("run protect --password");

    assert_eq!(out_prot.status.code(), Some(2));
    let json_prot: Value = serde_json::from_slice(&out_prot.stdout).expect("parse json");
    assert_eq!(json_prot["ok"], false);
    assert!(
        json_prot["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--password wurde entfernt (Sicherheitsrisiko)")
    );

    // 2. unlock --password
    let out_unl = sheaf_cmd()
        .args([
            "unlock",
            src.to_str().unwrap(),
            "--password",
            "legacy-pass",
            "--json",
        ])
        .output()
        .expect("run unlock --password");

    assert_eq!(out_unl.status.code(), Some(2));
    let json_unl: Value = serde_json::from_slice(&out_unl.stdout).expect("parse json");
    assert_eq!(json_unl["ok"], false);
    assert!(
        json_unl["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--password wurde entfernt (Sicherheitsrisiko)")
    );
}

#[test]
fn test_migration_legacy_owner_password_rejected() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_a_2p.pdf");

    let out_owner = sheaf_cmd()
        .args([
            "protect",
            src.to_str().unwrap(),
            "--owner-password",
            "legacy-owner",
            "--json",
        ])
        .output()
        .expect("run protect --owner-password");

    assert_eq!(out_owner.status.code(), Some(2));
    let json_owner: Value = serde_json::from_slice(&out_owner.stdout).expect("parse json");
    assert_eq!(json_owner["ok"], false);
    assert!(
        json_owner["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--owner-password wurde entfernt (Sicherheitsrisiko)")
    );
}

#[test]
fn test_migration_legacy_user_password_rejected() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_a_2p.pdf");

    // 1. protect --user-password
    let out_prot = sheaf_cmd()
        .args([
            "protect",
            src.to_str().unwrap(),
            "--user-password",
            "legacy-user",
            "--json",
        ])
        .output()
        .expect("run protect --user-password");

    assert_eq!(out_prot.status.code(), Some(2));
    let json_prot: Value = serde_json::from_slice(&out_prot.stdout).expect("parse json");
    assert_eq!(json_prot["ok"], false);
    assert!(
        json_prot["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--password wurde entfernt (Sicherheitsrisiko)")
    );

    // 2. unlock --user-password
    let out_unl = sheaf_cmd()
        .args([
            "unlock",
            src.to_str().unwrap(),
            "--user-password",
            "legacy-user",
            "--json",
        ])
        .output()
        .expect("run unlock --user-password");

    assert_eq!(out_unl.status.code(), Some(2));
    let json_unl: Value = serde_json::from_slice(&out_unl.stdout).expect("parse json");
    assert_eq!(json_unl["ok"], false);
    assert!(
        json_unl["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--password wurde entfernt (Sicherheitsrisiko)")
    );
}
