use std::path::{Path, PathBuf};
use std::process::Command;

use lopdf::Document;
use sheaf_core::{
    NoProgress, Progress, ProtectOptions, SheafError, UnlockOptions, WatermarkOptions,
    WatermarkPosition, info, protect, unlock, watermark,
};

struct CancelledProgress;

impl Progress for CancelledProgress {
    fn report(&self, _done: u64, _total: u64) {}
    fn is_cancelled(&self) -> bool {
        true
    }
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .join("tests")
        .join("fixtures")
}

fn validate_pdf(path: &Path, expected_pages: usize) -> Document {
    assert!(path.exists(), "PDF does not exist: {}", path.display());
    let doc = Document::load(path).expect("load output PDF");
    assert_eq!(doc.get_pages().len(), expected_pages);

    let qpdf_bin = ["qpdf", "/opt/homebrew/bin/qpdf", "/usr/local/bin/qpdf"]
        .into_iter()
        .find(|bin| {
            Command::new(bin)
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        });

    if let Some(qpdf) = qpdf_bin {
        let out = Command::new(qpdf)
            .arg("--check")
            .arg(path)
            .output()
            .expect("run qpdf --check");
        let code = out.status.code().unwrap_or(-1);
        assert!(
            code == 0 || code == 3,
            "qpdf --check failed (exit {code}) for {}:\n{}",
            path.display(),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    doc
}

#[test]
fn watermark_all_and_selected_pages_preserves_existing_text() {
    let src = fixtures_dir().join("fixture_b_3p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    // 1. Watermark all 3 pages diagonally
    let out_all = temp.path().join("wm_all.pdf");
    let rep_all = watermark(
        &src,
        &WatermarkOptions {
            text: "CONFIDENTIAL".to_string(),
            position: WatermarkPosition::Diagonal,
            font_size: 48.0,
            opacity: 0.25,
            rotation_deg: None,
            color: "red".to_string(),
            pages: None,
            out: Some(out_all.clone()),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect("watermark all");

    assert_eq!(rep_all.total_pages, 3);
    assert_eq!(rep_all.pages_watermarked, vec![1, 2, 3]);
    assert_eq!(rep_all.color, "#DC2626");
    let doc_all = validate_pdf(&out_all, 3);
    for p in 1..=3 {
        let txt = doc_all.extract_text(&[p]).expect("extract text");
        assert!(txt.contains("CONFIDENTIAL"), "page {p} missing watermark");
    }

    // 2. Watermark only pages 1,3 at bottom-right
    let out_sub = temp.path().join("wm_sub.pdf");
    let rep_sub = watermark(
        &src,
        &WatermarkOptions {
            text: "DRAFT COPY".to_string(),
            position: WatermarkPosition::BottomRight,
            font_size: 24.0,
            opacity: 0.40,
            rotation_deg: Some(0.0),
            color: "#2563EB".to_string(),
            pages: Some("1,3".to_string()),
            out: Some(out_sub.clone()),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect("watermark subset");

    assert_eq!(rep_sub.pages_watermarked, vec![1, 3]);
    let doc_sub = validate_pdf(&out_sub, 3);
    assert!(doc_sub.extract_text(&[1]).unwrap().contains("DRAFT COPY"));
    assert!(!doc_sub.extract_text(&[2]).unwrap().contains("DRAFT COPY"));
    assert!(doc_sub.extract_text(&[3]).unwrap().contains("DRAFT COPY"));
}

#[test]
fn protect_and_unlock_roundtrip_and_wrong_password() {
    let src = fixtures_dir().join("fixture_a_2p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    // 1. Encrypt with user and owner passwords
    let locked_pdf = temp.path().join("locked.pdf");
    let prot_rep = protect(
        &src,
        &ProtectOptions {
            user_password: secrecy::SecretString::from("user-pass-123"),
            owner_password: Some(secrecy::SecretString::from("owner-pass-456")),
            allow_print: true,
            allow_copy: false,
            allow_modify: true,
            out: Some(locked_pdf.clone()),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect("protect pdf");

    assert_eq!(prot_rep.action, "protect");
    assert_eq!(prot_rep.pages, 2);
    assert!(prot_rep.encrypted);
    assert!(locked_pdf.exists());

    // Confirm `info` reports encrypted = true
    let info_rep =
        info(std::slice::from_ref(&locked_pdf), &NoProgress).expect("info on locked pdf");
    assert!(info_rep.files[0].encrypted);
    assert_eq!(info_rep.files[0].pages, None);

    // 2. Unlock with wrong password fails with WrongPassword
    let bad_out = temp.path().join("bad_unlock.pdf");
    let bad_err = unlock(
        &locked_pdf,
        &UnlockOptions {
            password: secrecy::SecretString::from("wrong-password"),
            out: Some(bad_out.clone()),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect_err("wrong password must fail");
    assert!(
        matches!(bad_err, SheafError::WrongPassword),
        "expected WrongPassword, got: {bad_err:?}"
    );
    assert!(!bad_out.exists());

    // 3. Unlock with correct user password succeeds and produces valid unencrypted PDF
    let unlocked_pdf = temp.path().join("unlocked.pdf");
    let unl_rep = unlock(
        &locked_pdf,
        &UnlockOptions {
            password: secrecy::SecretString::from("user-pass-123"),
            out: Some(unlocked_pdf.clone()),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect("unlock with correct password");

    assert_eq!(unl_rep.action, "unlock");
    assert_eq!(unl_rep.pages, 2);
    assert!(!unl_rep.encrypted);

    let unlocked_doc = validate_pdf(&unlocked_pdf, 2);
    assert!(!unlocked_doc.is_encrypted());

    // 4. Calling unlock on an unencrypted PDF returns InvalidSelection
    let plain_err = unlock(
        &unlocked_pdf,
        &UnlockOptions {
            password: "any".to_string(),
            out: Some(temp.path().join("plain2.pdf")),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect_err("unlock on unencrypted pdf");
    assert!(matches!(plain_err, SheafError::InvalidSelection(_)));

    // 5. Cancellation check
    let cancel_err = watermark(
        &src,
        &WatermarkOptions {
            out: Some(temp.path().join("cancel.pdf")),
            ..Default::default()
        },
        &CancelledProgress,
    )
    .expect_err("cancelled");
    assert!(matches!(cancel_err, SheafError::Cancelled));
}
