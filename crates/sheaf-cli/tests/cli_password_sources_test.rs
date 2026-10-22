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
fn test_password_source_happy_paths_env_file_stdin() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_a_2p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    // File source
    let pw_file = temp.path().join("user_pass.txt");
    std::fs::write(&pw_file, b"secure-from-file\n").expect("write pw_file");

    let file_protected = temp.path().join("file_prot.pdf");
    let out_file_prot = sheaf_cmd()
        .args([
            "protect",
            src.to_str().unwrap(),
            "--new-password-file",
            pw_file.to_str().unwrap(),
            "-o",
            file_protected.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run protect --new-password-file");

    assert_eq!(out_file_prot.status.code(), Some(0));
    assert!(file_protected.exists());

    // Stdin source
    let stdin_unlocked = temp.path().join("stdin_unlocked.pdf");
    let out_stdin_unl = sheaf_cmd()
        .args([
            "unlock",
            file_protected.to_str().unwrap(),
            "--password-stdin",
            "-o",
            stdin_unlocked.to_str().unwrap(),
            "--json",
        ])
        .write_stdin("secure-from-file\r\n")
        .output()
        .expect("run unlock --password-stdin");

    assert_eq!(out_stdin_unl.status.code(), Some(0));
    assert!(stdin_unlocked.exists());

    // Env source
    let env_unlocked = temp.path().join("env_unlocked.pdf");
    let out_env_unl = sheaf_cmd()
        .env("CUSTOM_PDF_KEY", "secure-from-file")
        .args([
            "unlock",
            file_protected.to_str().unwrap(),
            "--password-env",
            "CUSTOM_PDF_KEY",
            "-o",
            env_unlocked.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run unlock --password-env");

    assert_eq!(out_env_unl.status.code(), Some(0));
    assert!(env_unlocked.exists());
}

#[test]
fn test_password_source_env_missing() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_a_2p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    let out = sheaf_cmd()
        .args([
            "protect",
            src.to_str().unwrap(),
            "--new-password-env",
            "DEFINITELY_NONEXISTENT_ENV_VAR_XYZ_987",
            "-o",
            temp.path().join("fail.pdf").to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run protect missing env");

    assert_eq!(out.status.code(), Some(2));
    let json: Value = serde_json::from_slice(&out.stdout).expect("parse json");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["exit_code"], 2);
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("not set"),
        "expected 'not set' in message: {}",
        json["error"]["message"]
    );
}

#[test]
fn test_password_source_env_empty() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_a_2p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    let out = sheaf_cmd()
        .env("EMPTY_PW_ENV_VAR", "")
        .args([
            "protect",
            src.to_str().unwrap(),
            "--new-password-env",
            "EMPTY_PW_ENV_VAR",
            "-o",
            temp.path().join("fail_empty.pdf").to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run protect empty env");

    assert_eq!(out.status.code(), Some(2));
    let json: Value = serde_json::from_slice(&out.stdout).expect("parse json");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["exit_code"], 2);
    assert!(
        json["error"]["message"].as_str().unwrap().contains("empty"),
        "expected 'empty' in message: {}",
        json["error"]["message"]
    );
}

#[test]
fn test_password_source_file_missing() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_a_2p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    let non_existent_file = temp.path().join("does_not_exist_pass.txt");

    let out = sheaf_cmd()
        .args([
            "protect",
            src.to_str().unwrap(),
            "--new-password-file",
            non_existent_file.to_str().unwrap(),
            "-o",
            temp.path().join("fail_missing_file.pdf").to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run protect missing file");

    assert_eq!(out.status.code(), Some(2));
    let json: Value = serde_json::from_slice(&out.stdout).expect("parse json");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["exit_code"], 2);
}

#[test]
fn test_password_source_file_empty() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_a_2p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    let empty_file = temp.path().join("empty_pw.txt");
    std::fs::write(&empty_file, b"").expect("write empty file");

    let out = sheaf_cmd()
        .args([
            "protect",
            src.to_str().unwrap(),
            "--new-password-file",
            empty_file.to_str().unwrap(),
            "-o",
            temp.path().join("fail_empty_file.pdf").to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run protect empty file");

    assert_eq!(out.status.code(), Some(2));
    let json: Value = serde_json::from_slice(&out.stdout).expect("parse json");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["exit_code"], 2);
    assert!(
        json["error"]["message"].as_str().unwrap().contains("empty"),
        "expected 'empty' in message: {}",
        json["error"]["message"]
    );
}

#[test]
fn test_password_source_two_stdin_flags() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_a_2p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    let out = sheaf_cmd()
        .args([
            "protect",
            src.to_str().unwrap(),
            "--password-stdin",
            "--new-password-stdin",
            "-o",
            temp.path().join("fail_multi_stdin.pdf").to_str().unwrap(),
            "--json",
        ])
        .write_stdin("first-pw\n")
        .output()
        .expect("run protect multi stdin");

    assert_eq!(out.status.code(), Some(2));
    let json: Value = serde_json::from_slice(&out.stdout).expect("parse json");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["exit_code"], 2);
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Only one password may be read from stdin"),
        "unexpected message: {}",
        json["error"]["message"]
    );
}

#[test]
#[cfg(unix)]
fn test_password_source_file_mode_0644_warns_on_unix() {
    use std::os::unix::fs::PermissionsExt;

    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_a_2p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    let pw_file = temp.path().join("pw_0644.txt");
    std::fs::write(&pw_file, b"secret123\n").expect("write pw_file");
    std::fs::set_permissions(&pw_file, std::fs::Permissions::from_mode(0o644))
        .expect("set mode 0644");

    let out = sheaf_cmd()
        .args([
            "protect",
            src.to_str().unwrap(),
            "--new-password-file",
            pw_file.to_str().unwrap(),
            "-o",
            temp.path().join("prot_0644.pdf").to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run protect with 0644 password file");

    assert_eq!(out.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("has loose permissions"),
        "Missing loose permissions warning in stderr: {stderr}"
    );

    let json: Value = serde_json::from_slice(&out.stdout).expect("parse json");
    let warnings = json["warnings"].as_array().expect("warnings array");
    assert!(
        warnings
            .iter()
            .any(|w| w.as_str().unwrap().contains("has loose permissions")),
        "Warning missing in json envelope: {warnings:?}"
    );
}
