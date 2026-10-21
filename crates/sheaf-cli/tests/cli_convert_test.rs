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
fn cli_convert_all_four_modes_end_to_end() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src_3p = fixtures.join("fixture_b_3p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    // 1. convert to-images
    let img_dir = temp.path().join("imgs");
    let out_to_img = sheaf_cmd()
        .args([
            "convert",
            "to-images",
            src_3p.to_str().unwrap(),
            "--format",
            "png",
            "--dpi",
            "150",
            "--pages",
            "1-2",
            "-o",
            img_dir.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run convert to-images");
    assert_eq!(out_to_img.status.code(), Some(0));
    let env_img: Value = serde_json::from_slice(&out_to_img.stdout).expect("parse json");
    assert_eq!(env_img["ok"], true);
    assert_eq!(env_img["op"], "convert");
    assert_eq!(env_img["data"]["action"], "to-images");
    assert_eq!(env_img["data"]["outputs"].as_array().unwrap().len(), 2);

    let p1_png = PathBuf::from(env_img["data"]["outputs"][0]["path"].as_str().unwrap());
    let p2_png = PathBuf::from(env_img["data"]["outputs"][1]["path"].as_str().unwrap());
    assert!(p1_png.exists());
    assert!(p2_png.exists());

    // 2. convert from-images
    let combined_pdf = temp.path().join("from_imgs.pdf");
    let out_from_img = sheaf_cmd()
        .args([
            "convert",
            "from-images",
            p1_png.to_str().unwrap(),
            p2_png.to_str().unwrap(),
            "--page-size",
            "a4",
            "--margin",
            "36",
            "-o",
            combined_pdf.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run convert from-images");
    assert_eq!(out_from_img.status.code(), Some(0));
    assert!(combined_pdf.exists());
    let env_from_img: Value = serde_json::from_slice(&out_from_img.stdout).expect("parse json");
    assert_eq!(env_from_img["data"]["action"], "from-images");
    assert_eq!(env_from_img["data"]["pages"], 2);

    // 3. convert from-markdown
    let md_in = temp.path().join("notes.md");
    std::fs::write(
        &md_in,
        "# CLI Markdown Test\n\n## Overview\n\n- Offline conversion\n- Vector text\n",
    )
    .expect("write notes.md");
    let md_pdf = temp.path().join("notes.pdf");
    let out_from_md = sheaf_cmd()
        .args([
            "convert",
            "from-markdown",
            md_in.to_str().unwrap(),
            "--page-size",
            "letter",
            "--margin",
            "54",
            "-o",
            md_pdf.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run convert from-markdown");
    assert_eq!(out_from_md.status.code(), Some(0));
    assert!(md_pdf.exists());
    let env_from_md: Value = serde_json::from_slice(&out_from_md.stdout).expect("parse json");
    assert_eq!(env_from_md["data"]["action"], "from-markdown");
    assert_eq!(env_from_md["data"]["headings_count"], 2);

    // 4. convert to-markdown
    let md_out = temp.path().join("roundtrip.md");
    let out_to_md = sheaf_cmd()
        .args([
            "convert",
            "to-markdown",
            md_pdf.to_str().unwrap(),
            "-o",
            md_out.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run convert to-markdown");
    assert_eq!(out_to_md.status.code(), Some(0));
    assert!(md_out.exists());
    let md_content = std::fs::read_to_string(&md_out).expect("read roundtrip.md");
    assert!(md_content.contains("# CLI Markdown Test"));
    assert!(md_content.contains("## Overview"));
    assert!(md_content.contains("- Offline conversion"));

    // 5. OutputExists without --force (exit code 7)
    let out_exists = sheaf_cmd()
        .args([
            "convert",
            "to-markdown",
            md_pdf.to_str().unwrap(),
            "-o",
            md_out.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run convert existing");
    assert_eq!(out_exists.status.code(), Some(7));
}
