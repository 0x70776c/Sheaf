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
fn test_password_sources_env_file_stdin_end_to_end() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_a_2p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    // 1. Protect using --new-password-file (with exactly one trailing newline to strip)
    let pw_file = temp.path().join("user_pass.txt");
    std::fs::write(&pw_file, b"secure-from-file\n").expect("write pw_file");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Set loose permissions (0o666) to trigger the warning
        std::fs::set_permissions(&pw_file, std::fs::Permissions::from_mode(0o666))
            .expect("set loose perms");
    }

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
    let stderr_file_prot = String::from_utf8_lossy(&out_file_prot.stderr);
    #[cfg(unix)]
    assert!(
        stderr_file_prot.contains("has loose permissions"),
        "Missing loose permissions warning: {stderr_file_prot}"
    );

    // 2. Unlock using --password-stdin
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

    // 3. Protect using --new-password-stdin
    let stdin_protected = temp.path().join("stdin_prot.pdf");
    let out_stdin_prot = sheaf_cmd()
        .args([
            "protect",
            src.to_str().unwrap(),
            "--new-password-stdin",
            "-o",
            stdin_protected.to_str().unwrap(),
            "--json",
        ])
        .write_stdin("piped-secret-pw\n")
        .output()
        .expect("run protect --new-password-stdin");

    assert_eq!(out_stdin_prot.status.code(), Some(0));
    assert!(stdin_protected.exists());

    // 4. Unlock using --password-env
    let env_unlocked = temp.path().join("env_unlocked.pdf");
    let out_env_unl = sheaf_cmd()
        .env("CUSTOM_PDF_KEY", "piped-secret-pw")
        .args([
            "unlock",
            stdin_protected.to_str().unwrap(),
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

    // 5. Error case: Missing env variable -> exit code 2
    let out_missing_env = sheaf_cmd()
        .args([
            "unlock",
            stdin_protected.to_str().unwrap(),
            "--password-env",
            "NONEXISTENT_ENV_VAR_12345",
            "-o",
            temp.path().join("fail.pdf").to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run unlock missing env");

    assert_eq!(out_missing_env.status.code(), Some(2));
    let env_err: Value = serde_json::from_slice(&out_missing_env.stdout).expect("parse json");
    assert_eq!(env_err["ok"], false);
    assert_eq!(env_err["error"]["exit_code"], 2);
    assert!(
        env_err["error"]["message"]
            .as_str()
            .unwrap()
            .contains("not set")
    );

    // 6. Error case: Empty env variable -> exit code 2
    let out_empty_env = sheaf_cmd()
        .env("EMPTY_ENV_VAR", "")
        .args([
            "unlock",
            stdin_protected.to_str().unwrap(),
            "--password-env",
            "EMPTY_ENV_VAR",
            "-o",
            temp.path().join("fail2.pdf").to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run unlock empty env");

    assert_eq!(out_empty_env.status.code(), Some(2));
    let empty_err: Value = serde_json::from_slice(&out_empty_env.stdout).expect("parse json");
    assert_eq!(empty_err["ok"], false);
    assert_eq!(empty_err["error"]["exit_code"], 2);
    assert!(
        empty_err["error"]["message"]
            .as_str()
            .unwrap()
            .contains("empty")
    );

    // 7. Error case: Empty password file -> exit code 2
    let empty_file = temp.path().join("empty.txt");
    std::fs::write(&empty_file, b"").expect("write empty file");
    let out_empty_file = sheaf_cmd()
        .args([
            "unlock",
            stdin_protected.to_str().unwrap(),
            "--password-file",
            empty_file.to_str().unwrap(),
            "-o",
            temp.path().join("fail3.pdf").to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run unlock empty file");

    assert_eq!(out_empty_file.status.code(), Some(2));
    let empty_file_err: Value = serde_json::from_slice(&out_empty_file.stdout).expect("parse json");
    assert_eq!(empty_file_err["ok"], false);
    assert!(
        empty_file_err["error"]["message"]
            .as_str()
            .unwrap()
            .contains("empty")
    );

    // 8. Error case: Multiple stdin flags -> exit code 2
    let out_multi_stdin = sheaf_cmd()
        .args([
            "protect",
            src.to_str().unwrap(),
            "--password-stdin",
            "--new-password-stdin",
            "-o",
            temp.path().join("fail4.pdf").to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run multi stdin");

    assert_eq!(out_multi_stdin.status.code(), Some(2));
}
