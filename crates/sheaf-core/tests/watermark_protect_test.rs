use std::path::{Path, PathBuf};
use std::process::Command;

use lopdf::Document;
use sheaf_core::{
    LoadOptions, NoProgress, Progress, ProtectOptions, SheafError, UnlockOptions, WatermarkOptions,
    WatermarkPosition, info, load_pdf, protect, unlock, watermark,
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
    let doc = load_pdf(path, &LoadOptions::none())
        .expect("load output PDF")
        .doc;
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
            load: LoadOptions::none(),
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
            load: LoadOptions::none(),
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
            load: LoadOptions::none(),
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

    if let Ok(qpdf) = Command::new("qpdf")
        .args(["--show-encryption", locked_pdf.to_str().unwrap()])
        .output()
    {
        let stdout = String::from_utf8_lossy(&qpdf.stdout);
        let stderr = String::from_utf8_lossy(&qpdf.stderr);
        println!("QPDF STDOUT:\n{stdout}\nQPDF STDERR:\n{stderr}");
    }

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
            password: Some(secrecy::SecretString::from("wrong-password")),
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
            password: Some(secrecy::SecretString::from("user-pass-123")),
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

    // Verify PDFium rendering behavior with password options
    let rend_no_pw = sheaf_core::render_with_options(
        &locked_pdf,
        &sheaf_core::RenderOptions {
            load: LoadOptions::none(),
            pages: Some("1".to_string()),
            scale: sheaf_core::RenderScale::WidthPx(120),
            out: temp.path().join("thumb_locked_no_pw.png"),
            force: false,
            dry_run: true,
            use_cache: false,
            cache_dir: None,
        },
        &NoProgress,
    );
    assert!(matches!(rend_no_pw, Err(SheafError::Encrypted)));

    let rend_wrong_pw = sheaf_core::render_with_options(
        &locked_pdf,
        &sheaf_core::RenderOptions {
            load: LoadOptions {
                password: Some(secrecy::SecretString::from("wrong")),
            },
            pages: Some("1".to_string()),
            scale: sheaf_core::RenderScale::WidthPx(120),
            out: temp.path().join("thumb_locked_wrong_pw.png"),
            force: false,
            dry_run: true,
            use_cache: false,
            cache_dir: None,
        },
        &NoProgress,
    );
    assert!(matches!(rend_wrong_pw, Err(SheafError::WrongPassword)));

    let rend_ok = sheaf_core::render_with_options(
        &locked_pdf,
        &sheaf_core::RenderOptions {
            load: LoadOptions {
                password: Some(secrecy::SecretString::from("user-pass-123")),
            },
            pages: Some("1".to_string()),
            scale: sheaf_core::RenderScale::WidthPx(120),
            out: temp.path().join("thumb_locked_ok.png"),
            force: false,
            dry_run: true,
            use_cache: false,
            cache_dir: None,
        },
        &NoProgress,
    );
    assert!(
        rend_ok.is_ok(),
        "PDFium failed to render encrypted PDF with user password: {rend_ok:?}"
    );

    // 4. Calling unlock on an unencrypted PDF succeeds per M10 spec (Exit 0, was_encrypted = false, output is copy)
    let plain_copy = temp.path().join("plain2.pdf");
    let plain_rep = unlock(
        &unlocked_pdf,
        &UnlockOptions {
            password: Some(secrecy::SecretString::from("any")),
            out: Some(plain_copy.clone()),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect("unlock on unencrypted pdf succeeds");
    assert!(!plain_rep.was_encrypted);
    assert_eq!(plain_rep.pages, 2);
    assert!(plain_copy.exists());

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

#[test]
#[allow(clippy::disallowed_methods, clippy::let_and_return)]
fn test_protect_permission_bits_and_qpdf_encryption_verification() {
    let src = fixtures_dir().join("fixture_a_2p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    // Case 1: No print
    let p_no_print = temp.path().join("no_print.pdf");
    protect(
        &src,
        &ProtectOptions {
            load: LoadOptions::none(),
            user_password: secrecy::SecretString::from("pw123"),
            owner_password: None,
            allow_print: false,
            allow_copy: true,
            allow_modify: true,
            out: Some(p_no_print.clone()),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect("protect no print");

    // Case 2: No copy
    let p_no_copy = temp.path().join("no_copy.pdf");
    protect(
        &src,
        &ProtectOptions {
            load: LoadOptions::none(),
            user_password: secrecy::SecretString::from("pw123"),
            owner_password: None,
            allow_print: true,
            allow_copy: false,
            allow_modify: true,
            out: Some(p_no_copy.clone()),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect("protect no copy");

    // Case 3: No modify
    let p_no_modify = temp.path().join("no_modify.pdf");
    protect(
        &src,
        &ProtectOptions {
            load: LoadOptions::none(),
            user_password: secrecy::SecretString::from("pw123"),
            owner_password: None,
            allow_print: true,
            allow_copy: true,
            allow_modify: false,
            out: Some(p_no_modify.clone()),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect("protect no modify");

    // Inspect /Encrypt /P values
    let get_p_val = |path: &Path| -> i64 {
        let bytes = std::fs::read(path).expect("read pdf bytes");
        let raw_doc = lopdf::Document::load_mem(&bytes).expect("load raw doc");
        let enc_obj = raw_doc
            .trailer
            .get(b"Encrypt")
            .expect("Encrypt entry in trailer");
        let enc_dict = match enc_obj {
            lopdf::Object::Reference(id) => raw_doc.get_object(*id).unwrap().as_dict().unwrap(),
            lopdf::Object::Dictionary(d) => d,
            _ => panic!("unexpected Encrypt obj"),
        };
        enc_dict.get(b"P").unwrap().as_i64().unwrap()
    };

    let p_val_no_print = get_p_val(&p_no_print);
    let p_val_no_copy = get_p_val(&p_no_copy);
    let p_val_no_modify = get_p_val(&p_no_modify);

    // Verify /P bits differ according to permissions set
    assert_ne!(p_val_no_print, p_val_no_copy);
    assert_ne!(p_val_no_print, p_val_no_modify);
    assert_ne!(p_val_no_copy, p_val_no_modify);

    // Verify qpdf --show-encryption confirms AES-256 (V=5, R=6) and permissions
    if let Ok(qpdf) = Command::new("qpdf")
        .args(["--show-encryption", p_no_print.to_str().unwrap()])
        .output()
    {
        let stdout = String::from_utf8_lossy(&qpdf.stdout);
        assert!(
            stdout.contains("AESv3") || stdout.contains("R = 6") || stdout.contains("256"),
            "qpdf should confirm 256-bit AES encryption (R=6, AESv3):\n{stdout}"
        );
        assert!(
            stdout.contains("print low resolution: not allowed")
                && stdout.contains("print high resolution: not allowed"),
            "qpdf should indicate print not allowed:\n{stdout}"
        );
    }

    if let Ok(qpdf) = Command::new("qpdf")
        .args(["--show-encryption", p_no_copy.to_str().unwrap()])
        .output()
    {
        let stdout = String::from_utf8_lossy(&qpdf.stdout);
        assert!(
            stdout.contains("extract for any purpose: not allowed"),
            "qpdf should indicate copy not allowed:\n{stdout}"
        );
    }

    if let Ok(qpdf) = Command::new("qpdf")
        .args(["--show-encryption", p_no_modify.to_str().unwrap()])
        .output()
    {
        let stdout = String::from_utf8_lossy(&qpdf.stdout);
        assert!(
            stdout.contains("modify annotations: not allowed")
                || stdout.contains("modify anything: not allowed"),
            "qpdf should indicate modify not allowed:\n{stdout}"
        );
    }
}

#[test]
fn test_protect_unlock_roundtrip_preserves_page_count_and_text() {
    let src = fixtures_dir().join("fixture_b_3p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    let orig_doc = load_pdf(&src, &LoadOptions::none()).expect("load orig").doc;
    let orig_pages = orig_doc.get_pages().len();
    let mut orig_texts = Vec::new();
    for p in 1..=orig_pages {
        orig_texts.push(
            orig_doc
                .extract_text(&[p as u32])
                .expect("extract orig text"),
        );
    }

    let locked_pdf = temp.path().join("roundtrip_locked.pdf");
    protect(
        &src,
        &ProtectOptions {
            load: LoadOptions::none(),
            user_password: secrecy::SecretString::from("roundtrip-pw"),
            owner_password: None,
            allow_print: true,
            allow_copy: true,
            allow_modify: true,
            out: Some(locked_pdf.clone()),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect("protect roundtrip");

    let unlocked_pdf = temp.path().join("roundtrip_unlocked.pdf");
    unlock(
        &locked_pdf,
        &UnlockOptions {
            password: Some(secrecy::SecretString::from("roundtrip-pw")),
            out: Some(unlocked_pdf.clone()),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect("unlock roundtrip");

    let unl_doc = load_pdf(&unlocked_pdf, &LoadOptions::none())
        .expect("load unl")
        .doc;
    assert_eq!(unl_doc.get_pages().len(), orig_pages);

    for p in 1..=orig_pages {
        let unl_txt = unl_doc.extract_text(&[p as u32]).expect("extract unl text");
        assert_eq!(
            unl_txt,
            orig_texts[p - 1],
            "Text on page {p} must match original"
        );
    }
}
