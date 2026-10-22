use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_version_plain() {
    let mut cmd = Command::cargo_bin("sheaf").unwrap();
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicate::str::starts_with("sheaf 0.1.0 (commit "));
}

#[test]
fn test_version_json() {
    let mut cmd = Command::cargo_bin("sheaf").unwrap();
    cmd.args(["--version", "--json"]);
    let assert = cmd.assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();

    let val: serde_json::Value = serde_json::from_str(&stdout).expect("Valid JSON version output");
    assert_eq!(val["schema"], 1);
    assert_eq!(val["version"], "0.1.0");
    assert!(val["commit"].is_string());
    assert!(val["date"].is_string());
    assert!(val["target"].is_string());
}

#[test]
fn test_completions_bash_zsh_fish() {
    for shell in ["bash", "zsh", "fish"] {
        let mut cmd = Command::cargo_bin("sheaf").unwrap();
        cmd.args(["completions", shell]);
        let assert = cmd.assert().success();
        let output = String::from_utf8(assert.get_output().stdout.clone()).unwrap();

        assert!(
            output.len() > 100,
            "Completion script for {shell} too short: {} bytes",
            output.len()
        );
        assert!(
            output.contains("sheaf"),
            "Completion script for {shell} missing 'sheaf'"
        );
    }
}

#[test]
fn test_completions_json() {
    let mut cmd = Command::cargo_bin("sheaf").unwrap();
    cmd.args(["completions", "zsh", "--json"]);
    let assert = cmd.assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();

    let val: serde_json::Value =
        serde_json::from_str(&stdout).expect("Valid completions JSON envelope");
    assert_eq!(val["schema"], 1);
    assert_eq!(val["ok"], true);
    assert_eq!(val["op"], "completions");
    assert_eq!(val["data"]["shell"], "zsh");
    let script = val["data"]["script"].as_str().expect("Script is string");
    assert!(script.len() > 100);
    assert!(script.contains("sheaf"));
}

#[test]
fn test_panic_hook_json_via_flag() {
    let mut cmd = Command::cargo_bin("sheaf").unwrap();
    cmd.args(["--test-panic", "--json"]);
    let assert = cmd.assert().code(1);
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();

    let val: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("Valid panic JSON envelope");
    assert_eq!(val["schema"], 1);
    assert_eq!(val["ok"], false);
    assert_eq!(val["op"], "panic");
    assert_eq!(val["error"]["code"], "internal");
    assert_eq!(val["error"]["exit_code"], 1);
    assert!(
        val["error"]["message"]
            .as_str()
            .unwrap()
            .contains("SHEAF_TEST_PANIC")
    );
}

#[test]
fn test_panic_hook_json_via_env() {
    let mut cmd = Command::cargo_bin("sheaf").unwrap();
    cmd.env("SHEAF_TEST_PANIC", "1").arg("--json");
    let assert = cmd.assert().code(1);
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();

    let val: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("Valid panic JSON envelope");
    assert_eq!(val["schema"], 1);
    assert_eq!(val["ok"], false);
    assert_eq!(val["op"], "panic");
    assert_eq!(val["error"]["code"], "internal");
    assert_eq!(val["error"]["exit_code"], 1);
}
