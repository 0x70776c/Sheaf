use std::path::PathBuf;

use assert_cmd::Command;
use sheaf_core::load::LoadOptions;
use sheaf_core::{NoProgress, render_thumbnails_with_options};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn sheaf_cmd() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("sheaf"))
}

const CANARY: &str = "LEAK-CANARY-7f3a";

fn assert_no_canary_leak(output: &std::process::Output, context: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !stdout.contains(CANARY),
        "SECURITY LEAK: Canary '{}' found in stdout ({context}):\n{stdout}",
        CANARY
    );
    assert!(
        !stderr.contains(CANARY),
        "SECURITY LEAK: Canary '{}' found in stderr ({context}):\n{stderr}",
        CANARY
    );
}

#[test]
fn test_security_secret_leak_canary_never_in_stdout_or_stderr() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_a_2p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    // 1. Setup canary in file and env
    let pw_file = temp.path().join("canary_pw.txt");
    std::fs::write(&pw_file, format!("{CANARY}\n")).expect("write canary pw file");

    let canary_env = "SHEAF_TEST_CANARY_PW";
    // Safety: single-threaded test environment
    unsafe { std::env::set_var(canary_env, CANARY) };

    let encrypted_pdf = temp.path().join("canary_enc.pdf");

    // Test combinations: protect with CANARY via file/env/stdin across various modes
    let protect_runs: Vec<(&str, Vec<String>, Option<&str>)> = vec![
        (
            "protect --new-password-env --json",
            vec![
                "protect".into(),
                src.to_str().unwrap().into(),
                "--new-password-env".into(),
                canary_env.into(),
                "-o".into(),
                encrypted_pdf.to_str().unwrap().into(),
                "--json".into(),
            ],
            None,
        ),
        (
            "protect --new-password-file --verbose",
            vec![
                "protect".into(),
                src.to_str().unwrap().into(),
                "--new-password-file".into(),
                pw_file.to_str().unwrap().into(),
                "-o".into(),
                temp.path().join("p_verb.pdf").to_str().unwrap().into(),
                "--verbose".into(),
                "--force".into(),
            ],
            None,
        ),
        (
            "protect --new-password-stdin --progress json",
            vec![
                "protect".into(),
                src.to_str().unwrap().into(),
                "--new-password-stdin".into(),
                "-o".into(),
                temp.path().join("p_prog.pdf").to_str().unwrap().into(),
                "--progress".into(),
                "json".into(),
                "--force".into(),
            ],
            Some(CANARY),
        ),
        (
            "protect --dry-run",
            vec![
                "protect".into(),
                src.to_str().unwrap().into(),
                "--new-password-env".into(),
                canary_env.into(),
                "--dry-run".into(),
                "-o".into(),
                temp.path().join("dry.pdf").to_str().unwrap().into(),
            ],
            None,
        ),
        (
            "protect error (missing input file with canary password)",
            vec![
                "protect".into(),
                temp.path()
                    .join("non_existent.pdf")
                    .to_str()
                    .unwrap()
                    .into(),
                "--new-password-env".into(),
                canary_env.into(),
            ],
            None,
        ),
    ];

    for (name, args, stdin_data) in protect_runs {
        let mut cmd = sheaf_cmd();
        cmd.args(&args);
        if let Some(data) = stdin_data {
            cmd.write_stdin(format!("{data}\n"));
        }
        let output = cmd.output().expect("run cmd");
        assert_no_canary_leak(&output, name);
    }

    assert!(encrypted_pdf.exists(), "canary_enc.pdf should be generated");

    // Reading commands to test: unlock, info, split, watermark
    let read_commands = ["unlock", "info", "split", "watermark"];

    for cmd_name in read_commands {
        // Test combinations: --json, --verbose, --progress json, --dry-run, error cases
        let test_cases: Vec<(String, Vec<String>, Option<&str>)> = vec![
            (
                format!("{cmd_name} --password-env --json"),
                match cmd_name {
                    "unlock" => vec![
                        cmd_name.into(),
                        encrypted_pdf.to_str().unwrap().into(),
                        "--password-env".into(),
                        canary_env.into(),
                        "-o".into(),
                        temp.path()
                            .join(format!("unlocked_{cmd_name}.pdf"))
                            .to_str()
                            .unwrap()
                            .into(),
                        "--json".into(),
                        "--force".into(),
                    ],
                    "info" => vec![
                        cmd_name.into(),
                        encrypted_pdf.to_str().unwrap().into(),
                        "--password-env".into(),
                        canary_env.into(),
                        "--json".into(),
                    ],
                    "split" => vec![
                        cmd_name.into(),
                        encrypted_pdf.to_str().unwrap().into(),
                        "--ranges".into(),
                        "1".into(),
                        "--password-env".into(),
                        canary_env.into(),
                        "-o".into(),
                        temp.path().join("split_dir").to_str().unwrap().into(),
                        "--json".into(),
                        "--force".into(),
                    ],
                    "watermark" => vec![
                        cmd_name.into(),
                        encrypted_pdf.to_str().unwrap().into(),
                        "--text".into(),
                        "CONFIDENTIAL".into(),
                        "--password-env".into(),
                        canary_env.into(),
                        "-o".into(),
                        temp.path().join("wm_out.pdf").to_str().unwrap().into(),
                        "--json".into(),
                        "--force".into(),
                    ],
                    _ => unreachable!(),
                },
                None,
            ),
            (
                format!("{cmd_name} --password-file --verbose"),
                match cmd_name {
                    "unlock" => vec![
                        cmd_name.into(),
                        encrypted_pdf.to_str().unwrap().into(),
                        "--password-file".into(),
                        pw_file.to_str().unwrap().into(),
                        "-o".into(),
                        temp.path().join("unl_verb.pdf").to_str().unwrap().into(),
                        "--verbose".into(),
                        "--force".into(),
                    ],
                    "info" => vec![
                        cmd_name.into(),
                        encrypted_pdf.to_str().unwrap().into(),
                        "--password-file".into(),
                        pw_file.to_str().unwrap().into(),
                        "--verbose".into(),
                    ],
                    "split" => vec![
                        cmd_name.into(),
                        encrypted_pdf.to_str().unwrap().into(),
                        "--ranges".into(),
                        "1".into(),
                        "--password-file".into(),
                        pw_file.to_str().unwrap().into(),
                        "-o".into(),
                        temp.path().join("split_verb").to_str().unwrap().into(),
                        "--verbose".into(),
                        "--force".into(),
                    ],
                    "watermark" => vec![
                        cmd_name.into(),
                        encrypted_pdf.to_str().unwrap().into(),
                        "--text".into(),
                        "CONFIDENTIAL".into(),
                        "--password-file".into(),
                        pw_file.to_str().unwrap().into(),
                        "-o".into(),
                        temp.path().join("wm_verb.pdf").to_str().unwrap().into(),
                        "--verbose".into(),
                        "--force".into(),
                    ],
                    _ => unreachable!(),
                },
                None,
            ),
            (
                format!("{cmd_name} --password-stdin --progress json"),
                match cmd_name {
                    "unlock" => vec![
                        cmd_name.into(),
                        encrypted_pdf.to_str().unwrap().into(),
                        "--password-stdin".into(),
                        "-o".into(),
                        temp.path().join("unl_prog.pdf").to_str().unwrap().into(),
                        "--progress".into(),
                        "json".into(),
                        "--force".into(),
                    ],
                    "info" => vec![
                        cmd_name.into(),
                        encrypted_pdf.to_str().unwrap().into(),
                        "--password-stdin".into(),
                        "--progress".into(),
                        "json".into(),
                    ],
                    "split" => vec![
                        cmd_name.into(),
                        encrypted_pdf.to_str().unwrap().into(),
                        "--ranges".into(),
                        "1".into(),
                        "--password-stdin".into(),
                        "-o".into(),
                        temp.path().join("split_prog").to_str().unwrap().into(),
                        "--progress".into(),
                        "json".into(),
                        "--force".into(),
                    ],
                    "watermark" => vec![
                        cmd_name.into(),
                        encrypted_pdf.to_str().unwrap().into(),
                        "--text".into(),
                        "CONFIDENTIAL".into(),
                        "--password-stdin".into(),
                        "-o".into(),
                        temp.path().join("wm_prog.pdf").to_str().unwrap().into(),
                        "--progress".into(),
                        "json".into(),
                        "--force".into(),
                    ],
                    _ => unreachable!(),
                },
                Some(CANARY),
            ),
            (
                format!("{cmd_name} --dry-run"),
                match cmd_name {
                    "unlock" => vec![
                        cmd_name.into(),
                        encrypted_pdf.to_str().unwrap().into(),
                        "--password-env".into(),
                        canary_env.into(),
                        "--dry-run".into(),
                        "-o".into(),
                        temp.path().join("unl_dry.pdf").to_str().unwrap().into(),
                    ],
                    "info" => vec![
                        cmd_name.into(),
                        encrypted_pdf.to_str().unwrap().into(),
                        "--password-env".into(),
                        canary_env.into(),
                        "--dry-run".into(),
                    ],
                    "split" => vec![
                        cmd_name.into(),
                        encrypted_pdf.to_str().unwrap().into(),
                        "--ranges".into(),
                        "1".into(),
                        "--password-env".into(),
                        canary_env.into(),
                        "--dry-run".into(),
                        "-o".into(),
                        temp.path().join("split_dry").to_str().unwrap().into(),
                    ],
                    "watermark" => vec![
                        cmd_name.into(),
                        encrypted_pdf.to_str().unwrap().into(),
                        "--text".into(),
                        "CONFIDENTIAL".into(),
                        "--password-env".into(),
                        canary_env.into(),
                        "--dry-run".into(),
                        "-o".into(),
                        temp.path().join("wm_dry.pdf").to_str().unwrap().into(),
                    ],
                    _ => unreachable!(),
                },
                None,
            ),
            (
                format!("{cmd_name} error case (invalid page range or bad arg with canary)"),
                match cmd_name {
                    "unlock" => vec![
                        cmd_name.into(),
                        encrypted_pdf.to_str().unwrap().into(),
                        "--password-env".into(),
                        canary_env.into(),
                        "--invalid-arg".into(),
                    ],
                    "info" => vec![
                        cmd_name.into(),
                        encrypted_pdf.to_str().unwrap().into(),
                        "--password-env".into(),
                        canary_env.into(),
                        "--invalid-arg".into(),
                    ],
                    "split" => vec![
                        cmd_name.into(),
                        encrypted_pdf.to_str().unwrap().into(),
                        "--ranges".into(),
                        "999".into(), // out of range
                        "--password-env".into(),
                        canary_env.into(),
                    ],
                    "watermark" => vec![
                        cmd_name.into(),
                        encrypted_pdf.to_str().unwrap().into(),
                        "--text".into(),
                        "CONFIDENTIAL".into(),
                        "--pages".into(),
                        "999".into(), // out of range
                        "--password-env".into(),
                        canary_env.into(),
                    ],
                    _ => unreachable!(),
                },
                None,
            ),
        ];

        for (name, args, stdin_data) in test_cases {
            let mut cmd = sheaf_cmd();
            cmd.args(&args);
            if let Some(data) = stdin_data {
                cmd.write_stdin(format!("{data}\n"));
            }
            let output = cmd.output().expect("run cmd");
            assert_no_canary_leak(&output, &name);
        }
    }
}

#[test]
fn test_encrypted_pdf_rendering_creates_no_disk_cache_files() {
    let fixtures = workspace_root().join("tests").join("fixtures");
    let src = fixtures.join("fixture_a_2p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    // 1. Encrypt PDF
    let enc_path = temp.path().join("cache_test_enc.pdf");
    let _ = sheaf_cmd()
        .args([
            "protect",
            src.to_str().unwrap(),
            "-o",
            enc_path.to_str().unwrap(),
            "--new-password-stdin",
        ])
        .write_stdin("canary-cache-pass\n")
        .assert()
        .success();

    // 2. Render thumbnails with dedicated cache directory
    let cache_dir = temp.path().join("secret_cache_dir");
    std::fs::create_dir_all(&cache_dir).expect("create cache dir");

    let load_opts = LoadOptions {
        password: Some(secrecy::SecretString::from("canary-cache-pass".to_string())),
    };

    let thumbs = render_thumbnails_with_options(
        &enc_path,
        &[1, 2],
        160,
        Some(&cache_dir),
        &load_opts,
        &NoProgress,
    )
    .expect("render encrypted thumbnails");

    assert_eq!(thumbs.len(), 2, "rendered 2 page thumbnails in RAM");

    // 3. Verify disk cache directory remains completely empty!
    let entries: Vec<_> = std::fs::read_dir(&cache_dir)
        .expect("read cache dir")
        .map(|e| e.unwrap().path())
        .collect();

    assert!(
        entries.is_empty(),
        "SECURITY LEAK: Encrypted PDF wrote files to thumbnail disk cache: {entries:?}"
    );
}
