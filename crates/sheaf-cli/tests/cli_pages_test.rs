use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use sheaf_core::get_effective_page_rotation;

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
fn cli_pages_subcommands_end_to_end() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src_3p = fixtures.join("fixture_b_3p.pdf");
    let src_12p = fixtures.join("fixture_12p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    // 1. pages rotate (additive: 90 + 180 = 270)
    let rot_1 = temp.path().join("rot1.pdf");
    let rot_2 = temp.path().join("rot2.pdf");

    sheaf_cmd()
        .args([
            "pages",
            "rotate",
            src_3p.to_str().unwrap(),
            "--pages",
            "1-2",
            "--degrees",
            "90",
            "-o",
            rot_1.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success();

    let out_rot2 = sheaf_cmd()
        .args([
            "pages",
            "rotate",
            rot_1.to_str().unwrap(),
            "--pages",
            "2-3",
            "--degrees",
            "-90",
            "-o",
            rot_2.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("rotate 2");
    assert_eq!(out_rot2.status.code(), Some(0));

    let doc_rot = sheaf_core::load_pdf(&rot_2, &sheaf_core::LoadOptions::none())
        .expect("load rot2")
        .doc;
    let pmap = doc_rot.get_pages();
    // page 1: 90 + 0 = 90
    // page 2: (90 + 270) % 360 = 0
    // page 3: 0 + 270 = 270
    assert_eq!(get_effective_page_rotation(&doc_rot, pmap[&1]), 90);
    assert_eq!(get_effective_page_rotation(&doc_rot, pmap[&2]), 0);
    assert_eq!(get_effective_page_rotation(&doc_rot, pmap[&3]), 270);

    // 2. pages delete
    let del_out = temp.path().join("deleted.pdf");
    sheaf_cmd()
        .args([
            "pages",
            "delete",
            src_12p.to_str().unwrap(),
            "--pages",
            "1-4,10-",
            "-o",
            del_out.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success();
    let doc_del = sheaf_core::load_pdf(&del_out, &sheaf_core::LoadOptions::none())
        .expect("load deleted")
        .doc;
    assert_eq!(doc_del.get_pages().len(), 5);

    // 3. pages reorder
    let reord_out = temp.path().join("reordered.pdf");
    sheaf_cmd()
        .args([
            "pages",
            "reorder",
            src_3p.to_str().unwrap(),
            "--order",
            "3,1,2",
            "-o",
            reord_out.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success();
    let doc_reord = sheaf_core::load_pdf(&reord_out, &sheaf_core::LoadOptions::none())
        .expect("load reordered")
        .doc;
    assert_eq!(doc_reord.get_pages().len(), 3);

    // 4. pages extract
    let ext_out = temp.path().join("extracted.pdf");
    sheaf_cmd()
        .args([
            "pages",
            "extract",
            src_12p.to_str().unwrap(),
            "--pages",
            "2,4,6",
            "-o",
            ext_out.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success();
    let doc_ext = sheaf_core::load_pdf(&ext_out, &sheaf_core::LoadOptions::none())
        .expect("load extracted")
        .doc;
    assert_eq!(doc_ext.get_pages().len(), 3);
}

#[test]
fn cli_pages_error_exit_codes_and_help() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src_3p = fixtures.join("fixture_b_3p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    // PageOutOfRange -> Exit 5
    let out = sheaf_cmd()
        .args([
            "pages",
            "rotate",
            src_3p.to_str().unwrap(),
            "--pages",
            "99",
            "--degrees",
            "90",
            "-o",
            temp.path().join("out.pdf").to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(5));
    let json: Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(json["error"]["code"], "page_out_of_range");

    // Invalid rotation angle (45°) -> Exit 2
    let out_deg = sheaf_cmd()
        .args([
            "pages",
            "rotate",
            src_3p.to_str().unwrap(),
            "--degrees",
            "45",
            "-o",
            temp.path().join("out.pdf").to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run");
    assert_eq!(out_deg.status.code(), Some(2));
    let json_deg: Value = serde_json::from_slice(&out_deg.stdout).expect("json");
    assert_eq!(json_deg["error"]["code"], "invalid_selection");

    // Deleting all pages -> Exit 2
    let out_del_all = sheaf_cmd()
        .args([
            "pages",
            "delete",
            src_3p.to_str().unwrap(),
            "--pages",
            "1-3",
            "-o",
            temp.path().join("out.pdf").to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run");
    assert_eq!(out_del_all.status.code(), Some(2));

    // Help examples for all 4 pages subcommands
    for sub in ["rotate", "delete", "reorder", "extract"] {
        sheaf_cmd()
            .args(["pages", sub, "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Examples:"));
    }
}

#[test]
fn cli_pages_snapshots() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::copy(
        fixtures.join("fixture_b_3p.pdf"),
        temp.path().join("input.pdf"),
    )
    .expect("copy");

    let rot_out = sheaf_cmd()
        .current_dir(temp.path())
        .args([
            "pages",
            "rotate",
            "input.pdf",
            "--pages",
            "1,3",
            "--degrees",
            "90",
            "-o",
            "rotated.pdf",
            "--json",
        ])
        .output()
        .expect("rotate");
    let rot_val: Value = serde_json::from_slice(&rot_out.stdout).expect("json");
    insta::assert_json_snapshot!("pages_rotate_envelope", rot_val, {
        ".duration_ms" => "[duration_ms]",
    });

    let ext_out = sheaf_cmd()
        .current_dir(temp.path())
        .args([
            "pages",
            "extract",
            "input.pdf",
            "--pages",
            "2-3",
            "-o",
            "extracted.pdf",
            "--json",
        ])
        .output()
        .expect("extract");
    let ext_val: Value = serde_json::from_slice(&ext_out.stdout).expect("json");
    insta::assert_json_snapshot!("pages_extract_envelope", ext_val, {
        ".duration_ms" => "[duration_ms]",
    });
}
