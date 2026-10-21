use std::path::{Path, PathBuf};

use assert_cmd::Command;
use lopdf::content::{Content, Operation};
use lopdf::{
    Bookmark, Document, EncryptionState, EncryptionVersion, Object, Permissions, Stream, dictionary,
};
use predicates::prelude::*;
use serde_json::Value;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn build_pdf(prefix: &str, page_count: u32, bookmarks: &[(&str, u32, Option<usize>)]) -> Document {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });

    let mut page_ids = Vec::with_capacity(page_count as usize);
    let mut kids = Vec::with_capacity(page_count as usize);

    for idx in 1..=page_count {
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 24.into()]),
                Operation::new("Td", vec![72.into(), 720.into()]),
                Operation::new(
                    "Tj",
                    vec![Object::string_literal(format!("{prefix} P{idx}"))],
                ),
                Operation::new("ET", vec![]),
            ],
        };
        let stream_id = doc.add_object(Stream::new(
            dictionary! {},
            content.encode().expect("encode"),
        ));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => stream_id,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });
        page_ids.push(page_id);
        kids.push(Object::Reference(page_id));
    }

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => i64::from(page_count),
        }),
    );

    let info_id = doc.add_object(dictionary! {
        "Title" => Object::string_literal(prefix),
        "Author" => Object::string_literal("Sheaf Test"),
    });

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    doc.trailer.set("Info", info_id);

    let mut added_ids: Vec<u32> = Vec::new();
    for &(title, page_num, parent_idx) in bookmarks {
        let target_id = page_ids[(page_num - 1) as usize];
        let parent_id = parent_idx.map(|i| added_ids[i]);
        let bm = Bookmark::new(title.to_string(), [0.0, 0.0, 0.0], 0, target_id);
        added_ids.push(doc.add_bookmark(bm, parent_id));
    }

    if let Some(outline_id) = doc.build_outline()
        && let Ok(Object::Dictionary(cat)) = doc.get_object_mut(catalog_id)
    {
        cat.set("Outlines", Object::Reference(outline_id));
        cat.set("PageMode", Object::Name(b"UseOutlines".to_vec()));
    }

    doc
}

fn ensure_all_fixtures() -> PathBuf {
    let dir = workspace_root().join("tests").join("fixtures");
    std::fs::create_dir_all(&dir).expect("create fixtures dir");

    if !dir.join("fixture_12p.pdf").exists() {
        let mut doc = build_pdf("Report12", 12, &[]);
        doc.save(dir.join("fixture_12p.pdf")).expect("save 12p");
    }

    if !dir.join("fixture_no_outlines.pdf").exists() {
        let mut doc = build_pdf("Plain", 4, &[]);
        doc.save(dir.join("fixture_no_outlines.pdf"))
            .expect("save no_outlines");
    }

    if !dir.join("fixture_outlines.pdf").exists() {
        let outlines = [
            ("1. Einführung", 2, None),
            ("1.1 Grundlagen", 2, Some(0)),
            ("1.2 Details", 4, Some(0)),
            ("2. Hauptteil", 6, None),
            ("2.1 Analyse", 6, Some(3)),
            ("2.2 Ergebnisse", 8, Some(3)),
            ("3. Ausblick & Fazit", 10, None),
            ("3.1 Zusammenfassung", 10, Some(6)),
            ("3.2 Nächste Schritte", 11, Some(6)),
        ];
        let mut doc = build_pdf("Book", 12, &outlines);
        doc.save(dir.join("fixture_outlines.pdf"))
            .expect("save outlines");
    }

    if !dir.join("corrupted.pdf").exists() {
        std::fs::write(dir.join("corrupted.pdf"), b"%PDF-1.5\nbroken xref")
            .expect("save corrupted");
    }

    if !dir.join("encrypted.pdf").exists() {
        let mut enc_doc = build_pdf("Secret", 1, &[]);
        enc_doc.trailer.set(
            "ID",
            Object::Array(vec![
                Object::string_literal(b"0123456789abcdef".to_vec()),
                Object::string_literal(b"0123456789abcdef".to_vec()),
            ]),
        );
        let state = EncryptionState::try_from(EncryptionVersion::V2 {
            document: &enc_doc,
            owner_password: "owner-secret",
            user_password: "user-secret",
            key_length: 128,
            permissions: Permissions::PRINTABLE,
        })
        .expect("enc state");
        enc_doc.encrypt(&state).expect("encrypt");
        enc_doc
            .save(dir.join("encrypted.pdf"))
            .expect("save encrypted");
    }

    dir
}

fn sheaf_cmd() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("sheaf"))
}

#[test]
fn cli_exit_code_table_driven() {
    let fixtures = ensure_all_fixtures();
    let temp = tempfile::tempdir().expect("tempdir");

    let existing_out = temp.path().join("existing.pdf");
    std::fs::write(&existing_out, b"already here").expect("write existing");

    // Pre-create one of the expected split outputs to trigger Exit 7 on split without --force
    let split_conflict_dir = temp.path().join("conflict_dir");
    std::fs::create_dir_all(&split_conflict_dir).expect("create conflict dir");
    std::fs::write(
        split_conflict_dir.join("fixture_12p_p1-3.pdf"),
        b"existing split output",
    )
    .expect("write existing split output");

    let f12 = fixtures.join("fixture_12p.pdf");
    let f_corrupt = fixtures.join("corrupted.pdf");
    let f_enc = fixtures.join("encrypted.pdf");
    let f_no_outlines = fixtures.join("fixture_no_outlines.pdf");
    let f_missing = temp.path().join("does_not_exist.pdf");

    struct Case {
        name: &'static str,
        args: Vec<String>,
        expected_exit: i32,
        expected_code: &'static str,
    }

    let cases = vec![
        Case {
            name: "usage error: clap missing required flag",
            args: vec!["split".into(), f12.display().to_string(), "--json".into()],
            expected_exit: 2,
            expected_code: "invalid_selection",
        },
        Case {
            name: "usage error: --combine without --ranges",
            args: vec![
                "split".into(),
                f12.display().to_string(),
                "--every".into(),
                "2".into(),
                "--combine".into(),
                "--json".into(),
            ],
            expected_exit: 2,
            expected_code: "invalid_selection",
        },
        Case {
            name: "invalid selection: reversed range 5-3",
            args: vec![
                "split".into(),
                f12.display().to_string(),
                "--ranges".into(),
                "5-3".into(),
                "--json".into(),
            ],
            expected_exit: 2,
            expected_code: "invalid_selection",
        },
        Case {
            name: "empty selection: empty range string",
            args: vec![
                "split".into(),
                f12.display().to_string(),
                "--ranges".into(),
                "   ".into(),
                "--json".into(),
            ],
            expected_exit: 2,
            expected_code: "empty_selection",
        },
        Case {
            name: "corrupted PDF -> exit 3",
            args: vec![
                "info".into(),
                f_corrupt.display().to_string(),
                "--json".into(),
            ],
            expected_exit: 3,
            expected_code: "invalid_pdf",
        },
        Case {
            name: "verify corrupted PDF -> exit 3",
            args: vec![
                "verify".into(),
                f_corrupt.display().to_string(),
                "--json".into(),
            ],
            expected_exit: 3,
            expected_code: "invalid_pdf",
        },
        Case {
            name: "encrypted PDF on split -> exit 4",
            args: vec![
                "split".into(),
                f_enc.display().to_string(),
                "--every".into(),
                "1".into(),
                "--json".into(),
            ],
            expected_exit: 4,
            expected_code: "encrypted",
        },
        Case {
            name: "page out of range (99 on 12-page PDF) -> exit 5",
            args: vec![
                "split".into(),
                f12.display().to_string(),
                "--ranges".into(),
                "99".into(),
                "--json".into(),
            ],
            expected_exit: 5,
            expected_code: "page_out_of_range",
        },
        Case {
            name: "missing input file -> exit 6 (io)",
            args: vec![
                "info".into(),
                f_missing.display().to_string(),
                "--json".into(),
            ],
            expected_exit: 6,
            expected_code: "io",
        },
        Case {
            name: "output exists on merge without --force -> exit 7",
            args: vec![
                "merge".into(),
                f12.display().to_string(),
                "-o".into(),
                existing_out.display().to_string(),
                "--json".into(),
            ],
            expected_exit: 7,
            expected_code: "output_exists",
        },
        Case {
            name: "output exists on split without --force -> exit 7",
            args: vec![
                "split".into(),
                f12.display().to_string(),
                "--ranges".into(),
                "1-3".into(),
                "-o".into(),
                split_conflict_dir.display().to_string(),
                "--json".into(),
            ],
            expected_exit: 7,
            expected_code: "output_exists",
        },
        Case {
            name: "unsupported: split by bookmarks on PDF without bookmarks -> exit 8",
            args: vec![
                "split".into(),
                f_no_outlines.display().to_string(),
                "--bookmarks-level".into(),
                "1".into(),
                "-o".into(),
                temp.path().display().to_string(),
                "--json".into(),
            ],
            expected_exit: 8,
            expected_code: "unsupported",
        },
    ];

    for c in cases {
        let output = sheaf_cmd()
            .args(&c.args)
            .output()
            .unwrap_or_else(|e| panic!("failed to run case {}: {e}", c.name));

        assert_eq!(
            output.status.code(),
            Some(c.expected_exit),
            "Exit code mismatch for case '{}'. stdout: {}, stderr: {}",
            c.name,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let json: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
            panic!(
                "stdout is not valid single JSON object for case '{}': {e}\nstdout: {}",
                c.name,
                String::from_utf8_lossy(&output.stdout)
            )
        });

        assert_eq!(json["schema"], 1, "case: {}", c.name);
        assert_eq!(json["ok"], false, "case: {}", c.name);
        assert_eq!(json["error"]["code"], c.expected_code, "case: {}", c.name);
        assert_eq!(
            json["error"]["exit_code"], c.expected_exit,
            "case: {}",
            c.name
        );
    }
}

#[test]
fn cli_info_on_encrypted_pdf_succeeds_with_exit_0() {
    let fixtures = ensure_all_fixtures();
    let f_enc = fixtures.join("encrypted.pdf");

    let output = sheaf_cmd()
        .args(["info", f_enc.to_str().unwrap(), "--json"])
        .output()
        .expect("run info on encrypted.pdf");

    assert_eq!(output.status.code(), Some(0));
    let json: Value = serde_json::from_slice(&output.stdout).expect("parse json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["files"][0]["encrypted"], true);
    assert!(json["data"]["files"][0]["pages"].is_null());
}

#[test]
fn cli_stdout_purity_and_progress_ndjson_on_stderr() {
    let fixtures = ensure_all_fixtures();
    let f12 = fixtures.join("fixture_12p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    let output = sheaf_cmd()
        .args([
            "split",
            f12.to_str().unwrap(),
            "--ranges",
            "1-3,5,8-",
            "-o",
            temp.path().to_str().unwrap(),
            "--json",
            "--progress",
            "json",
        ])
        .output()
        .expect("run split with --json and --progress json");

    assert_eq!(output.status.code(), Some(0));

    // 1. stdout must parse as a single JSON object
    let stdout_json: Value =
        serde_json::from_slice(&output.stdout).expect("stdout must be a single JSON value");
    assert_eq!(stdout_json["schema"], 1);
    assert_eq!(stdout_json["ok"], true);
    assert_eq!(stdout_json["op"], "split");

    // 2. stderr must contain NDJSON progress events
    let stderr_str = String::from_utf8_lossy(&output.stderr);
    let lines: Vec<&str> = stderr_str.lines().collect();
    assert!(
        !lines.is_empty(),
        "Expected NDJSON progress lines on stderr"
    );
    for line in lines {
        let evt: Value = serde_json::from_str(line).expect("each stderr line must be valid JSON");
        assert_eq!(evt["event"], "progress");
        assert_eq!(evt["op"], "split");
    }
}

#[test]
fn cli_dry_run_writes_no_files_and_matches_plan() {
    let fixtures = ensure_all_fixtures();
    let f12 = fixtures.join("fixture_12p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");
    let out_dir = temp.path().join("dry_out");

    // 1. Run with --dry-run
    let dry_out = sheaf_cmd()
        .args([
            "split",
            f12.to_str().unwrap(),
            "--ranges",
            "1-3,8-",
            "-o",
            out_dir.to_str().unwrap(),
            "--dry-run",
            "--json",
        ])
        .output()
        .expect("run dry-run split");

    assert_eq!(dry_out.status.code(), Some(0));
    assert!(
        !out_dir.exists(),
        "--dry-run must not create the output directory or any files"
    );

    let dry_json: Value = serde_json::from_slice(&dry_out.stdout).expect("parse dry json");
    assert_eq!(dry_json["dry_run"], true);

    // 2. Run without --dry-run
    let real_out = sheaf_cmd()
        .args([
            "split",
            f12.to_str().unwrap(),
            "--ranges",
            "1-3,8-",
            "-o",
            out_dir.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run real split");

    assert_eq!(real_out.status.code(), Some(0));
    let real_json: Value = serde_json::from_slice(&real_out.stdout).expect("parse real json");
    assert_eq!(real_json["dry_run"], false);

    // Plan in `data` must be identical between dry-run and real run
    assert_eq!(dry_json["data"], real_json["data"]);
}

#[test]
fn cli_subcommands_help_contain_examples() {
    for sub in [
        "merge",
        "split",
        "pages",
        "render",
        "compress",
        "convert",
        "info",
        "verify",
        "capabilities",
    ] {
        sheaf_cmd()
            .args([sub, "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Examples:"));
    }
}

#[test]
fn cli_json_envelope_snapshots() {
    let fixtures = ensure_all_fixtures();
    let temp = tempfile::tempdir().expect("tempdir");

    // Copy fixture into temp dir and run with current_dir(temp) so paths in snapshots are relative & stable
    let local_report = temp.path().join("report.pdf");
    std::fs::copy(fixtures.join("fixture_12p.pdf"), &local_report).expect("copy report.pdf");

    // 1. capabilities --json
    let cap_out = sheaf_cmd()
        .current_dir(temp.path())
        .args(["capabilities", "--json"])
        .output()
        .expect("capabilities");
    let cap_val: Value = serde_json::from_slice(&cap_out.stdout).expect("json");
    insta::assert_json_snapshot!("capabilities_envelope", cap_val, {
        ".duration_ms" => "[duration_ms]",
        ".data.tools.qpdf" => true,
    });

    // 2. info --json
    let info_out = sheaf_cmd()
        .current_dir(temp.path())
        .args(["info", "report.pdf", "--json"])
        .output()
        .expect("info");
    let info_val: Value = serde_json::from_slice(&info_out.stdout).expect("json");
    insta::assert_json_snapshot!("info_envelope", info_val, {
        ".duration_ms" => "[duration_ms]",
        ".data.files[].size_bytes" => "[size_bytes]",
    });

    // 3. split --json
    let split_out = sheaf_cmd()
        .current_dir(temp.path())
        .args([
            "split",
            "report.pdf",
            "--ranges",
            "1-3,5,8-",
            "-o",
            "out",
            "--json",
        ])
        .output()
        .expect("split");
    let split_val: Value = serde_json::from_slice(&split_out.stdout).expect("json");
    insta::assert_json_snapshot!("split_envelope", split_val, {
        ".duration_ms" => "[duration_ms]",
    });

    // 4. merge --json
    let merge_out = sheaf_cmd()
        .current_dir(temp.path())
        .args([
            "merge",
            "out/report_p1-3.pdf",
            "out/report_p5.pdf",
            "-o",
            "merged.pdf",
            "--json",
        ])
        .output()
        .expect("merge");
    let merge_val: Value = serde_json::from_slice(&merge_out.stdout).expect("json");
    insta::assert_json_snapshot!("merge_envelope", merge_val, {
        ".duration_ms" => "[duration_ms]",
    });

    // 5. verify --json
    let verify_out = sheaf_cmd()
        .current_dir(temp.path())
        .args(["verify", "merged.pdf", "--json"])
        .output()
        .expect("verify");
    let verify_val: Value = serde_json::from_slice(&verify_out.stdout).expect("json");
    insta::assert_json_snapshot!("verify_envelope", verify_val, {
        ".duration_ms" => "[duration_ms]",
    });
}
