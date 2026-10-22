use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use lopdf::content::{Content, Operation};
use lopdf::{
    Bookmark, Document, EncryptionState, EncryptionVersion, Object, Permissions, Stream, dictionary,
};
use sheaf_core::{LoadOptions, NoProgress, Progress, SheafError, load_pdf, merge};

static INIT_FIXTURES: Once = Once::new();

fn fixtures_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .join("tests")
        .join("fixtures")
}

/// Builds a valid PDF document with `page_count` pages and optional top-level/nested bookmarks.
fn build_sample_pdf(
    title_prefix: &str,
    page_count: u32,
    bookmarks: &[(&str, u32, Option<usize>)],
) -> Document {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! {
            "F1" => font_id,
        },
    });

    let mut page_ids = Vec::with_capacity(page_count as usize);
    let mut kids = Vec::with_capacity(page_count as usize);

    for page_idx in 1..=page_count {
        let label = format!("{title_prefix} - Page {page_idx}");
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 24.into()]),
                Operation::new("Td", vec![72.into(), 720.into()]),
                Operation::new("Tj", vec![Object::string_literal(label)]),
                Operation::new("ET", vec![]),
            ],
        };
        let content_bytes = content.encode().expect("encode content stream");
        let content_id = doc.add_object(Stream::new(dictionary! {}, content_bytes));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });
        page_ids.push(page_id);
        kids.push(Object::Reference(page_id));
    }

    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => kids,
        "Count" => i64::from(page_count),
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    // Add bookmarks if requested: (title, 1-based page number, optional parent index in `added_ids`)
    let mut added_ids: Vec<u32> = Vec::with_capacity(bookmarks.len());
    for &(bm_title, page_num, parent_idx) in bookmarks {
        let target_page_id = page_ids[(page_num - 1) as usize];
        let parent_bm_id = parent_idx.map(|idx| added_ids[idx]);
        let bm = Bookmark::new(bm_title.to_string(), [0.0, 0.0, 0.0], 0, target_page_id);
        let id = doc.add_bookmark(bm, parent_bm_id);
        added_ids.push(id);
    }

    if let Some(outline_id) = doc.build_outline()
        && let Ok(Object::Dictionary(cat)) = doc.get_object_mut(catalog_id)
    {
        cat.set("Outlines", Object::Reference(outline_id));
        cat.set("PageMode", Object::Name(b"UseOutlines".to_vec()));
    }

    doc
}

/// Ensures all test fixtures in `tests/fixtures/` exist (creating them programmatically via `lopdf`).
fn ensure_fixtures() -> PathBuf {
    let dir = fixtures_dir();
    INIT_FIXTURES.call_once(|| {
        std::fs::create_dir_all(&dir).expect("create tests/fixtures directory");

        // Fixture 1: 2 pages, with bookmarks (root + child)
        let mut doc1 = build_sample_pdf(
            "DocA",
            2,
            &[("DocA Intro", 1, None), ("DocA Details", 2, Some(0))],
        );
        doc1.save(dir.join("fixture_a_2p.pdf"))
            .expect("save fixture_a_2p.pdf");

        // Fixture 2: 3 pages, with bookmarks
        let mut doc2 = build_sample_pdf(
            "DocB",
            3,
            &[("DocB Start", 1, None), ("DocB Conclusion", 3, None)],
        );
        doc2.save(dir.join("fixture_b_3p.pdf"))
            .expect("save fixture_b_3p.pdf");

        // Fixture 3: 1 page, with bookmark
        let mut doc3 = build_sample_pdf("DocC", 1, &[("DocC Appendix", 1, None)]);
        doc3.save(dir.join("fixture_c_1p.pdf"))
            .expect("save fixture_c_1p.pdf");

        // Fixture 4: corrupted PDF
        std::fs::write(dir.join("corrupted.pdf"), b"%PDF-1.5\nnot a valid pdf xref")
            .expect("write corrupted.pdf");

        // Fixture 5: encrypted PDF
        if !dir.join("encrypted.pdf").exists() {
            let mut enc_doc = build_sample_pdf("Secret", 1, &[]);
            // Add deterministic file ID required by V2 encryption
            enc_doc.trailer.set(
                "ID",
                Object::Array(vec![
                    Object::string_literal(b"0123456789abcdef".to_vec()),
                    Object::string_literal(b"0123456789abcdef".to_vec()),
                ]),
            );
            let enc_version = EncryptionVersion::V2 {
                document: &enc_doc,
                owner_password: "owner-secret",
                user_password: "user-secret",
                key_length: 128,
                permissions: Permissions::PRINTABLE,
            };
            let state = EncryptionState::try_from(enc_version).expect("build encryption state");
            enc_doc.encrypt(&state).expect("encrypt document");
            enc_doc
                .save(dir.join("encrypted.pdf"))
                .expect("save encrypted.pdf");
        }
    });
    dir
}

/// Validates a generated PDF file:
/// 1. Loads with `lopdf` and asserts the page count equals `expected_pages`.
/// 2. If `qpdf` is installed on the system, runs `qpdf --check <path>` and asserts exit status 0.
fn validate_output_pdf(path: &Path, expected_pages: usize) -> Document {
    assert!(
        path.exists(),
        "Output PDF does not exist: {}",
        path.display()
    );
    let doc = load_pdf(path, &LoadOptions::none())
        .expect("Output PDF should load cleanly with lopdf")
        .doc;
    assert_eq!(
        doc.get_pages().len(),
        expected_pages,
        "Page count mismatch in {}",
        path.display()
    );

    // Check with qpdf if available on PATH or in /opt/homebrew/bin/qpdf
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
        let output = Command::new(qpdf)
            .arg("--check")
            .arg(path)
            .output()
            .expect("Failed to execute qpdf --check");

        // qpdf exit code 0 = no errors/warnings, 3 = warnings only (still valid PDF)
        let code = output.status.code().unwrap_or(-1);
        assert!(
            code == 0 || code == 3,
            "qpdf --check failed (exit code {code}) for {}:\nstdout: {}\nstderr: {}",
            path.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    doc
}

#[test]
fn merge_three_fixtures_preserves_pages_and_bookmarks() {
    let dir = ensure_fixtures();
    let f1 = dir.join("fixture_a_2p.pdf");
    let f2 = dir.join("fixture_b_3p.pdf");
    let f3 = dir.join("fixture_c_1p.pdf");

    // Record original file sizes/contents to verify originals are untouched
    let orig_f1 = std::fs::read(&f1).expect("read f1");
    let orig_f2 = std::fs::read(&f2).expect("read f2");
    let orig_f3 = std::fs::read(&f3).expect("read f3");

    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let out_path = temp_dir.path().join("merged_output.pdf");

    let inputs = vec![f1.clone(), f2.clone(), f3.clone()];
    merge(&inputs, &out_path, &NoProgress).expect("merge 3 fixtures should succeed");

    // 1. Verify page count = 2 + 3 + 1 = 6 and validate with qpdf
    let merged_doc = validate_output_pdf(&out_path, 6);

    // 2. Verify bookmarks/outlines are preserved and point to the right shifted page numbers
    let toc = merged_doc
        .get_toc()
        .expect("merged PDF should contain outlines/TOC");
    let summary: Vec<(usize, String, usize)> = toc
        .toc
        .into_iter()
        .map(|entry| (entry.level, entry.title, entry.page))
        .collect();

    assert_eq!(
        summary,
        vec![
            (1, "DocA Intro".to_string(), 1),
            (2, "DocA Details".to_string(), 2),
            (1, "DocB Start".to_string(), 3),
            (1, "DocB Conclusion".to_string(), 5),
            (1, "DocC Appendix".to_string(), 6),
        ]
    );

    // 3. Verify original files were never modified
    assert_eq!(std::fs::read(&f1).expect("read f1"), orig_f1);
    assert_eq!(std::fs::read(&f2).expect("read f2"), orig_f2);
    assert_eq!(std::fs::read(&f3).expect("read f3"), orig_f3);
}

#[test]
fn merge_empty_inputs_returns_empty_selection() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let out_path = temp_dir.path().join("out.pdf");

    let err = merge(&[], &out_path, &NoProgress).expect_err("empty inputs must fail");
    assert!(matches!(err, SheafError::EmptySelection));
}

#[test]
fn merge_corrupted_pdf_returns_invalid_pdf() {
    let dir = ensure_fixtures();
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let out_path = temp_dir.path().join("out.pdf");

    let inputs = vec![dir.join("fixture_a_2p.pdf"), dir.join("corrupted.pdf")];
    let err = merge(&inputs, &out_path, &NoProgress).expect_err("corrupted input must fail");
    assert!(matches!(err, SheafError::InvalidPdf(_)));
}

#[test]
fn merge_encrypted_pdf_returns_encrypted_error() {
    let dir = ensure_fixtures();
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let out_path = temp_dir.path().join("out.pdf");

    let inputs = vec![dir.join("fixture_a_2p.pdf"), dir.join("encrypted.pdf")];
    let err = merge(&inputs, &out_path, &NoProgress).expect_err("encrypted input must fail");
    assert!(
        matches!(err, SheafError::Encrypted),
        "Expected SheafError::Encrypted, got: {err:?}"
    );
    assert_eq!(err.to_string(), "File is password-protected");
}

#[test]
fn merge_refuses_to_overwrite_input_file() {
    let dir = ensure_fixtures();
    let f1 = dir.join("fixture_a_2p.pdf");
    let f2 = dir.join("fixture_b_3p.pdf");

    let err = merge(&[f1.clone(), f2], &f1, &NoProgress).expect_err("overwriting input must fail");
    assert!(matches!(err, SheafError::InvalidSelection(_)));
}

#[test]
fn merge_supports_progress_and_cancellation() {
    struct TestProgress {
        calls: AtomicU64,
        cancel_after: u64,
        cancelled: AtomicBool,
    }

    impl Progress for TestProgress {
        fn report(&self, done: u64, _total: u64) {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if done >= self.cancel_after {
                self.cancelled.store(true, Ordering::SeqCst);
            }
        }

        fn is_cancelled(&self) -> bool {
            self.cancelled.load(Ordering::SeqCst)
        }
    }

    let dir = ensure_fixtures();
    let inputs = vec![
        dir.join("fixture_a_2p.pdf"),
        dir.join("fixture_b_3p.pdf"),
        dir.join("fixture_c_1p.pdf"),
    ];
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let out_path = temp_dir.path().join("cancelled.pdf");

    let progress = TestProgress {
        calls: AtomicU64::new(0),
        cancel_after: 1,
        cancelled: AtomicBool::new(false),
    };

    let err = merge(&inputs, &out_path, &progress).expect_err("should cancel");
    assert!(matches!(err, SheafError::Cancelled));
    assert!(!out_path.exists());
}
