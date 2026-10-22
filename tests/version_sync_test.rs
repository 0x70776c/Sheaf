use std::fs;
use std::path::Path;

#[test]
fn test_version_sync_across_workspace() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("Failed to locate workspace root");

    // 1. Cargo.toml
    let cargo_toml =
        fs::read_to_string(root.join("Cargo.toml")).expect("Failed to read Cargo.toml");
    let cargo_version = cargo_toml
        .lines()
        .find(|l| l.trim().starts_with("version ="))
        .and_then(|l| l.split('"').nth(1))
        .expect("Failed to parse version from Cargo.toml");

    // 2. tauri.conf.json
    let tauri_conf_path = root.join("apps/desktop/src-tauri/tauri.conf.json");
    let tauri_conf_str =
        fs::read_to_string(&tauri_conf_path).expect("Failed to read tauri.conf.json");
    let tauri_val: serde_json::Value =
        serde_json::from_str(&tauri_conf_str).expect("Failed to parse tauri.conf.json");
    let tauri_version = tauri_val
        .get("version")
        .and_then(|v| v.as_str())
        .expect("Missing version in tauri.conf.json");

    // 3. package.json
    let package_json_path = root.join("apps/desktop/package.json");
    let package_json_str =
        fs::read_to_string(&package_json_path).expect("Failed to read package.json");
    let package_val: serde_json::Value =
        serde_json::from_str(&package_json_str).expect("Failed to parse package.json");
    let package_version = package_val
        .get("version")
        .and_then(|v| v.as_str())
        .expect("Missing version in package.json");

    assert_eq!(
        cargo_version, tauri_version,
        "Version mismatch between Cargo.toml ({cargo_version}) and tauri.conf.json ({tauri_version})"
    );
    assert_eq!(
        cargo_version, package_version,
        "Version mismatch between Cargo.toml ({cargo_version}) and package.json ({package_version})"
    );
    assert_eq!(
        cargo_version, "0.1.0",
        "Expected release version 0.1.0, got {cargo_version}"
    );
}
