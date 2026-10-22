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
fn test_password_pipeline_matrix_across_all_reading_commands() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_a_2p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    let enc_pdf = temp.path().join("matrix_enc.pdf");
    let correct_pw = "matrix-pass-123";
    let wrong_pw = "incorrect-pass-999";

    // 1. Create encrypted PDF
    let prot_out = sheaf_cmd()
        .args([
            "protect",
            src.to_str().unwrap(),
            "-o",
            enc_pdf.to_str().unwrap(),
            "--new-password-stdin",
        ])
        .write_stdin(format!("{correct_pw}\n"))
        .output()
        .expect("protect file");
    assert_eq!(prot_out.status.code(), Some(0));
    assert!(enc_pdf.exists());

    let merged_out = temp.path().join("merged.pdf");
    let split_out = temp.path().join("split_out");
    let rot_out = temp.path().join("rot.pdf");
    let del_out = temp.path().join("del.pdf");
    let reord_out = temp.path().join("reord.pdf");
    let ext_out = temp.path().join("ext.pdf");
    let comp_out = temp.path().join("comp.pdf");
    let img_out = temp.path().join("images_out");
    let md_out = temp.path().join("out.md");
    let wm_out = temp.path().join("wm.pdf");
    let num_out = temp.path().join("num.pdf");
    let unl_out = temp.path().join("unl.pdf");
    let rend_out = temp.path().join("render_out");

    let enc_path = enc_pdf.to_str().unwrap();

    let reading_commands: Vec<(&str, Vec<String>, bool)> = vec![
        (
            "merge",
            vec![
                "merge".into(),
                enc_path.into(),
                "-o".into(),
                merged_out.to_str().unwrap().into(),
            ],
            true,
        ),
        (
            "split",
            vec![
                "split".into(),
                enc_path.into(),
                "--ranges".into(),
                "1".into(),
                "-o".into(),
                split_out.to_str().unwrap().into(),
            ],
            true,
        ),
        (
            "pages rotate",
            vec![
                "pages".into(),
                "rotate".into(),
                enc_path.into(),
                "--degrees".into(),
                "90".into(),
                "-o".into(),
                rot_out.to_str().unwrap().into(),
            ],
            true,
        ),
        (
            "pages delete",
            vec![
                "pages".into(),
                "delete".into(),
                enc_path.into(),
                "--pages".into(),
                "2".into(),
                "-o".into(),
                del_out.to_str().unwrap().into(),
            ],
            true,
        ),
        (
            "pages reorder",
            vec![
                "pages".into(),
                "reorder".into(),
                enc_path.into(),
                "--order".into(),
                "2,1".into(),
                "-o".into(),
                reord_out.to_str().unwrap().into(),
            ],
            true,
        ),
        (
            "pages extract",
            vec![
                "pages".into(),
                "extract".into(),
                enc_path.into(),
                "--pages".into(),
                "1".into(),
                "-o".into(),
                ext_out.to_str().unwrap().into(),
            ],
            true,
        ),
        (
            "compress",
            vec![
                "compress".into(),
                enc_path.into(),
                "-o".into(),
                comp_out.to_str().unwrap().into(),
            ],
            true,
        ),
        (
            "convert to-images",
            vec![
                "convert".into(),
                "to-images".into(),
                enc_path.into(),
                "-o".into(),
                img_out.to_str().unwrap().into(),
            ],
            true,
        ),
        (
            "convert to-markdown",
            vec![
                "convert".into(),
                "to-markdown".into(),
                enc_path.into(),
                "-o".into(),
                md_out.to_str().unwrap().into(),
            ],
            true,
        ),
        (
            "watermark",
            vec![
                "watermark".into(),
                enc_path.into(),
                "--text".into(),
                "CONFIDENTIAL".into(),
                "-o".into(),
                wm_out.to_str().unwrap().into(),
            ],
            true,
        ),
        (
            "number",
            vec![
                "number".into(),
                enc_path.into(),
                "-o".into(),
                num_out.to_str().unwrap().into(),
            ],
            true,
        ),
        (
            "unlock",
            vec![
                "unlock".into(),
                enc_path.into(),
                "-o".into(),
                unl_out.to_str().unwrap().into(),
            ],
            true,
        ),
        (
            "render",
            vec![
                "render".into(),
                enc_path.into(),
                "-o".into(),
                rend_out.to_str().unwrap().into(),
            ],
            false,
        ),
        ("verify", vec!["verify".into(), enc_path.into()], false),
    ];

    for (name, base_args, produces_file) in reading_commands {
        // CASE A: No password provided -> Exit 4, code: encrypted
        let mut no_pw_args = base_args.clone();
        no_pw_args.push("--json".into());
        no_pw_args.push("--force".into());

        let out_no_pw = sheaf_cmd()
            .args(&no_pw_args)
            .output()
            .unwrap_or_else(|e| panic!("run {name} without password failed: {e}"));

        assert_eq!(
            out_no_pw.status.code(),
            Some(4),
            "Command '{name}' without password expected exit code 4, got {:?}. Stderr:\n{}",
            out_no_pw.status.code(),
            String::from_utf8_lossy(&out_no_pw.stderr)
        );
        let json_no_pw: Value = serde_json::from_slice(&out_no_pw.stdout).unwrap_or_else(|e| {
            panic!(
                "parse json for {name} (no pw): {e}\nRaw stdout: {}",
                String::from_utf8_lossy(&out_no_pw.stdout)
            )
        });
        assert_eq!(
            json_no_pw["ok"], false,
            "{name} without password ok should be false"
        );
        assert_eq!(
            json_no_pw["error"]["code"], "encrypted",
            "{name} without password error code should be encrypted"
        );

        // CASE B: Wrong password provided -> Exit 4, code: wrong_password
        let mut wrong_pw_args = base_args.clone();
        wrong_pw_args.push("--password-stdin".into());
        wrong_pw_args.push("--json".into());
        wrong_pw_args.push("--force".into());

        let out_wrong_pw = sheaf_cmd()
            .args(&wrong_pw_args)
            .write_stdin(format!("{wrong_pw}\n"))
            .output()
            .unwrap_or_else(|e| panic!("run {name} with wrong password failed: {e}"));

        assert_eq!(
            out_wrong_pw.status.code(),
            Some(4),
            "Command '{name}' with wrong password expected exit code 4, got {:?}. Stderr:\n{}",
            out_wrong_pw.status.code(),
            String::from_utf8_lossy(&out_wrong_pw.stderr)
        );
        let json_wrong_pw: Value = serde_json::from_slice(&out_wrong_pw.stdout)
            .unwrap_or_else(|e| panic!("parse json for {name} (wrong pw): {e}"));
        assert_eq!(json_wrong_pw["ok"], false);
        assert_eq!(
            json_wrong_pw["error"]["code"], "wrong_password",
            "{name} with wrong password error code should be wrong_password"
        );

        // CASE C: Correct password provided -> Exit 0, and if produces file, warning about decrypted output
        let mut correct_pw_args = base_args.clone();
        correct_pw_args.push("--password-stdin".into());
        correct_pw_args.push("--json".into());
        correct_pw_args.push("--force".into());

        let out_correct = sheaf_cmd()
            .args(&correct_pw_args)
            .write_stdin(format!("{correct_pw}\n"))
            .output()
            .unwrap_or_else(|e| panic!("run {name} with correct password failed: {e}"));

        assert_eq!(
            out_correct.status.code(),
            Some(0),
            "Command '{name}' with correct password expected exit code 0, got {:?}. Stderr:\n{}",
            out_correct.status.code(),
            String::from_utf8_lossy(&out_correct.stderr)
        );
        let json_correct: Value = serde_json::from_slice(&out_correct.stdout)
            .unwrap_or_else(|e| panic!("parse json for {name} (correct pw): {e}"));
        assert_eq!(
            json_correct["ok"], true,
            "{name} with correct password should succeed"
        );

        if produces_file {
            let warnings = json_correct["warnings"]
                .as_array()
                .expect("warnings array in json");
            assert!(
                warnings.iter().any(|w| w
                    .as_str()
                    .unwrap()
                    .contains("Eingabe war verschlüsselt, Ausgabe ist nicht verschlüsselt")),
                "Command '{name}' should emit warning that output is unencrypted. Warnings: {warnings:?}"
            );
        }
    }
}
