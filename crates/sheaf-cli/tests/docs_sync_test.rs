use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;

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
fn test_docs_sync_cli_md_matches_generated() {
    let root = workspace_root();
    let docs_file = root.join("docs").join("CLI.md");

    let out = sheaf_cmd().arg("docs").output().expect("run sheaf docs");
    assert_eq!(out.status.code(), Some(0));

    let generated = String::from_utf8(out.stdout).expect("utf8 stdout");

    if std::env::var("UPDATE_DOCS").is_ok() || !docs_file.exists() {
        if let Some(parent) = docs_file.parent() {
            fs::create_dir_all(parent).expect("create docs dir");
        }
        fs::write(&docs_file, &generated).expect("write docs/CLI.md");
    }

    let existing = fs::read_to_string(&docs_file).unwrap_or_default();
    assert_eq!(
        existing, generated,
        "docs/CLI.md is out of sync with Clap CLI definition! Run `UPDATE_DOCS=1 cargo test --test docs_sync_test` to update it."
    );
}

#[test]
fn test_readme_links_cli_docs_and_contains_all_command_examples() {
    let root = workspace_root();
    let readme_path = root.join("README.md");
    let readme_content = fs::read_to_string(&readme_path).expect("read README.md");

    // 1. README must link docs/CLI.md
    assert!(
        readme_content.contains("docs/CLI.md"),
        "README.md must link to docs/CLI.md"
    );

    // 2. Fetch all command names from capabilities
    let out = sheaf_cmd()
        .args(["capabilities", "--json"])
        .output()
        .expect("run sheaf capabilities");
    assert_eq!(out.status.code(), Some(0));

    let val: serde_json::Value = serde_json::from_slice(&out.stdout).expect("parse json");
    let commands = val["data"]["commands"].as_array().expect("commands array");

    for cmd in commands {
        let name = cmd["name"].as_str().expect("command name");
        let search_pattern = format!("sheaf {name}");
        assert!(
            readme_content.contains(&search_pattern),
            "README.md must contain at least one example for command 'sheaf {name}'"
        );
    }
}
