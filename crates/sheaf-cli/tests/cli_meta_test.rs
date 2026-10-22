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

#[test]
fn test_cli_meta_set_updates_fields_and_preserves_page_contents() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_b_3p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    // 1. Dry run creates no file
    let dry_out = temp.path().join("dry_out.pdf");
    let out_dry = sheaf_cmd()
        .args([
            "meta",
            "set",
            src.to_str().unwrap(),
            "--title",
            "Updated Title",
            "-o",
            dry_out.to_str().unwrap(),
            "--dry-run",
            "--json",
        ])
        .output()
        .expect("run meta set dry run");

    assert_eq!(out_dry.status.code(), Some(0));
    assert!(
        !dry_out.exists(),
        "--dry-run must not create the output file"
    );

    // 2. Real update: set standard fields, custom key, and unset Producer
    let updated_pdf = temp.path().join("updated.pdf");
    let out_set = sheaf_cmd()
        .args([
            "meta",
            "set",
            src.to_str().unwrap(),
            "--title",
            "My New Report",
            "--author",
            "Dr. Alice",
            "--subject",
            "Financial Audit",
            "--keywords",
            "audit, finance, 2026",
            "--custom",
            "Department=Treasury",
            "--custom",
            "Security_Classification=Confidential",
            "--unset",
            "Producer",
            "-o",
            updated_pdf.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run meta set");

    assert_eq!(out_set.status.code(), Some(0));
    assert!(updated_pdf.exists());

    // 3. Inspect via `meta get` to confirm fields
    let out_get = sheaf_cmd()
        .args(["meta", "get", updated_pdf.to_str().unwrap(), "--json"])
        .output()
        .expect("run meta get on updated.pdf");

    assert_eq!(out_get.status.code(), Some(0));
    let json_get: Value = serde_json::from_slice(&out_get.stdout).expect("parse json");
    let file_info = &json_get["data"]["files"][0];
    assert_eq!(file_info["info"]["title"], "My New Report");
    assert_eq!(file_info["info"]["author"], "Dr. Alice");
    assert_eq!(file_info["info"]["subject"], "Financial Audit");
    assert_eq!(file_info["info"]["keywords"], "audit, finance, 2026");
    assert_eq!(file_info["info"]["producer"], Value::Null);
    assert_eq!(file_info["custom"]["Department"], "Treasury");
    assert_eq!(
        file_info["custom"]["Security_Classification"],
        "Confidential"
    );

    // 4. Verify page contents are identical
    let orig_loaded = sheaf_core::load_pdf(&src, &sheaf_core::LoadOptions::none()).unwrap();
    let updated_loaded =
        sheaf_core::load_pdf(&updated_pdf, &sheaf_core::LoadOptions::none()).unwrap();

    let orig_pages = orig_loaded.doc.get_pages();
    let updated_pages = updated_loaded.doc.get_pages();
    assert_eq!(orig_pages.len(), updated_pages.len());

    for (p_num, (&orig_id, &upd_id)) in orig_pages.values().zip(updated_pages.values()).enumerate()
    {
        let orig_contents = orig_loaded.doc.get_page_content(orig_id);
        let upd_contents = updated_loaded.doc.get_page_content(upd_id);
        assert_eq!(
            orig_contents,
            upd_contents,
            "Page {} content bytes must be byte-identical!",
            p_num + 1
        );
    }
}

#[test]
fn test_cli_meta_set_validation_errors() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_a_2p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    // Case A: No changes specified -> Exit 2
    let out_no_change = sheaf_cmd()
        .args([
            "meta",
            "set",
            src.to_str().unwrap(),
            "-o",
            temp.path().join("fail1.pdf").to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run meta set no change");
    assert_eq!(out_no_change.status.code(), Some(2));

    // Case B: Empty value -> Exit 2
    let out_empty = sheaf_cmd()
        .args([
            "meta",
            "set",
            src.to_str().unwrap(),
            "--title",
            "",
            "-o",
            temp.path().join("fail2.pdf").to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run meta set empty value");
    assert_eq!(out_empty.status.code(), Some(2));

    // Case C: Custom key conflicts with standard key -> Exit 2
    let out_conflict = sheaf_cmd()
        .args([
            "meta",
            "set",
            src.to_str().unwrap(),
            "--custom",
            "Title=Conflicts",
            "-o",
            temp.path().join("fail3.pdf").to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run meta set conflict");
    assert_eq!(out_conflict.status.code(), Some(2));
}

#[test]
fn test_cli_meta_strip_default_and_keep() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_a_2p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");
    let stripped_path = temp.path().join("stripped.pdf");

    let out = sheaf_cmd()
        .args([
            "meta",
            "strip",
            src.to_str().unwrap(),
            "-o",
            stripped_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run meta strip");
    assert!(out.status.success(), "meta strip must succeed");

    let val: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("parse json output from meta strip");
    assert_eq!(val["ok"], true);
    let removed = &val["data"]["removed"];
    assert!(removed["trailer_id_regenerated"].as_bool().unwrap());
    assert!(stripped_path.exists());

    // Inspect stripped file: Info dictionary should be completely removed
    let opts = sheaf_core::LoadOptions::default();
    let loaded = sheaf_core::load_pdf(&stripped_path, &opts).expect("load stripped pdf");
    assert!(
        loaded.doc.trailer.get(b"Info").is_err(),
        "Info dictionary should be completely stripped when no keys kept"
    );

    // Now test with --keep Title
    let kept_path = temp.path().join("kept.pdf");
    let out_keep = sheaf_cmd()
        .args([
            "meta",
            "strip",
            src.to_str().unwrap(),
            "--keep",
            "title",
            "-o",
            kept_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run meta strip keep");
    assert!(out_keep.status.success());
    let loaded_kept = sheaf_core::load_pdf(&kept_path, &opts).expect("load kept pdf");
    if let Ok(info_obj) = loaded_kept.doc.trailer.get(b"Info")
        && let Ok(info_dict) = loaded_kept
            .doc
            .get_dictionary(info_obj.as_reference().unwrap())
    {
        assert!(info_dict.has(b"Title"), "Title should be kept");
    }
}

#[test]
fn test_cli_meta_strip_jpeg_warning_and_deep() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_photo.pdf");
    let temp = tempfile::tempdir().expect("tempdir");
    let stripped_path = temp.path().join("stripped_photo.pdf");

    let out = sheaf_cmd()
        .args([
            "meta",
            "strip",
            src.to_str().unwrap(),
            "--deep",
            "-o",
            stripped_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run meta strip photo");
    assert!(out.status.success());

    let val: serde_json::Value = serde_json::from_slice(&out.stdout).expect("parse json");
    let warnings = val["warnings"].as_array().expect("warnings array");
    assert!(
        warnings.iter().any(|w| w
            .as_str()
            .unwrap()
            .contains("JPEG-Bilder können EXIF-Daten enthalten")),
        "Must contain JPEG EXIF warning"
    );
}

#[test]
fn test_cli_meta_strip_dry_run() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_a_2p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");
    let target = temp.path().join("should_not_exist.pdf");

    let out = sheaf_cmd()
        .args([
            "meta",
            "strip",
            src.to_str().unwrap(),
            "--dry-run",
            "-o",
            target.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run meta strip dry-run");
    assert!(out.status.success());
    assert!(!target.exists(), "Dry run must not create output file");
}

#[test]
fn test_cli_meta_output_exists_exit_7() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_a_2p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");
    let existing_path = temp.path().join("existing.pdf");
    std::fs::write(&existing_path, b"%PDF-1.4 dummy\n").expect("write existing");

    let out = sheaf_cmd()
        .args([
            "meta",
            "set",
            src.to_str().unwrap(),
            "--title",
            "Conflict Title",
            "-o",
            existing_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run meta set target exists");
    assert_eq!(out.status.code(), Some(7), "Existing output without --force must exit 7");
}

#[test]
fn test_cli_meta_multi_get_corrupted_file_exit_3() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src_ok = fixtures.join("fixture_a_2p.pdf");
    let src_corrupt = fixtures.join("corrupted.pdf");

    let out = sheaf_cmd()
        .args([
            "meta",
            "get",
            src_ok.to_str().unwrap(),
            src_corrupt.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run meta get with corrupted file");

    assert_eq!(out.status.code(), Some(3), "Corrupted PDF in meta get must return exit 3");
    let val: serde_json::Value = serde_json::from_slice(&out.stdout).expect("parse json");
    assert_eq!(val["ok"], false);
    assert_eq!(val["error"]["code"], "invalid_pdf");
}

#[test]
fn test_cli_meta_secret_leak_canary() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_a_2p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");
    let canary = "CANARY_META_PW_SECRET_987654";

    // 1. Lock a file with the canary
    let locked_path = temp.path().join("locked.pdf");
    let prot_out = sheaf_cmd()
        .args([
            "protect",
            src.to_str().unwrap(),
            "--new-password-stdin",
            "-o",
            locked_path.to_str().unwrap(),
        ])
        .write_stdin(format!("{canary}\n"))
        .output()
        .expect("protect with canary");
    assert!(prot_out.status.success());

    // 2. meta get with canary password
    let get_out = sheaf_cmd()
        .args([
            "meta",
            "get",
            locked_path.to_str().unwrap(),
            "--password-stdin",
            "--json",
        ])
        .write_stdin(format!("{canary}\n"))
        .output()
        .expect("meta get with canary");
    assert!(get_out.status.success());
    let get_stdout = String::from_utf8_lossy(&get_out.stdout);
    let get_stderr = String::from_utf8_lossy(&get_out.stderr);
    assert!(!get_stdout.contains(canary), "Canary must not leak to stdout in meta get");
    assert!(!get_stderr.contains(canary), "Canary must not leak to stderr in meta get");

    // 3. meta set with canary password
    let set_out_path = temp.path().join("set_unlocked.pdf");
    let set_out = sheaf_cmd()
        .args([
            "meta",
            "set",
            locked_path.to_str().unwrap(),
            "--title",
            "New Secret Title",
            "--password-stdin",
            "-o",
            set_out_path.to_str().unwrap(),
            "--json",
        ])
        .write_stdin(format!("{canary}\n"))
        .output()
        .expect("meta set with canary");
    assert!(set_out.status.success());
    let set_stdout = String::from_utf8_lossy(&set_out.stdout);
    let set_stderr = String::from_utf8_lossy(&set_out.stderr);
    assert!(!set_stdout.contains(canary), "Canary must not leak to stdout in meta set");
    assert!(!set_stderr.contains(canary), "Canary must not leak to stderr in meta set");

    // 4. meta strip with canary password
    let strip_out_path = temp.path().join("strip_unlocked.pdf");
    let strip_out = sheaf_cmd()
        .args([
            "meta",
            "strip",
            locked_path.to_str().unwrap(),
            "--password-stdin",
            "-o",
            strip_out_path.to_str().unwrap(),
            "--json",
        ])
        .write_stdin(format!("{canary}\n"))
        .output()
        .expect("meta strip with canary");
    assert!(strip_out.status.success());
    let strip_stdout = String::from_utf8_lossy(&strip_out.stdout);
    let strip_stderr = String::from_utf8_lossy(&strip_out.stderr);
    assert!(!strip_stdout.contains(canary), "Canary must not leak to stdout in meta strip");
    assert!(!strip_stderr.contains(canary), "Canary must not leak to stderr in meta strip");
}
