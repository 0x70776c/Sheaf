use std::path::{Path, PathBuf};
use std::process::Command;

use lopdf::Document;
use sheaf_core::{
    NoProgress, PageOpOptions, PageSelection, SheafError, delete_pages, extract_pages,
    get_effective_page_rotation, normalize_rotation_degrees, reorder_pages, rotate_pages,
};

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
fn normalize_degrees_valid_and_invalid() {
    assert_eq!(normalize_rotation_degrees(90).expect("90"), 90);
    assert_eq!(normalize_rotation_degrees(180).expect("180"), 180);
    assert_eq!(normalize_rotation_degrees(270).expect("270"), 270);
    assert_eq!(normalize_rotation_degrees(-90).expect("-90"), 270);
    assert_eq!(normalize_rotation_degrees(-180).expect("-180"), 180);
    assert_eq!(normalize_rotation_degrees(-270).expect("-270"), 90);

    for bad in [0, 45, 100, 360, -45] {
        assert!(matches!(
            normalize_rotation_degrees(bad),
            Err(SheafError::InvalidSelection(_))
        ));
    }
}

#[test]
fn rotate_pages_is_additive_and_validates_bounds() {
    let src = fixtures_dir().join("fixture_b_3p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    // Step 1: Rotate pages 1 and 3 by 90 degrees
    let out1 = temp.path().join("rot1.pdf");
    let sel1 = PageSelection::parse("1,3", 3).expect("parse");
    let opts1 = PageOpOptions {
        out: Some(out1.clone()),
        force: false,
        dry_run: false,
    };
    let rep1 = rotate_pages(&src, Some(&sel1), 90, &opts1, &NoProgress).expect("rotate 1");
    assert_eq!(rep1.action, "rotate");
    assert_eq!(rep1.input_pages, 3);
    assert_eq!(rep1.output_pages, 3);
    assert_eq!(rep1.pages_affected, vec![1, 3]);
    assert_eq!(rep1.degrees, Some(90));

    let doc1 = validate_pdf(&out1, 3);
    let pages1 = doc1.get_pages();
    assert_eq!(get_effective_page_rotation(&doc1, pages1[&1]), 90);
    assert_eq!(get_effective_page_rotation(&doc1, pages1[&2]), 0);
    assert_eq!(get_effective_page_rotation(&doc1, pages1[&3]), 90);

    // Step 2: Rotate all pages of rot1.pdf by 180 degrees -> page 1 = 270, page 2 = 180, page 3 = 270
    let out2 = temp.path().join("rot2.pdf");
    let opts2 = PageOpOptions {
        out: Some(out2.clone()),
        force: false,
        dry_run: false,
    };
    rotate_pages(&out1, None, 180, &opts2, &NoProgress).expect("rotate 2");
    let doc2 = validate_pdf(&out2, 3);
    let pages2 = doc2.get_pages();
    assert_eq!(get_effective_page_rotation(&doc2, pages2[&1]), 270);
    assert_eq!(get_effective_page_rotation(&doc2, pages2[&2]), 180);
    assert_eq!(get_effective_page_rotation(&doc2, pages2[&3]), 270);

    // Step 3: Rotate page 1 by 90 degrees -> (270 + 90) % 360 = 0
    let out3 = temp.path().join("rot3.pdf");
    let sel3 = PageSelection::parse("1", 3).expect("parse");
    let opts3 = PageOpOptions {
        out: Some(out3.clone()),
        force: false,
        dry_run: false,
    };
    rotate_pages(&out2, Some(&sel3), 90, &opts3, &NoProgress).expect("rotate 3");
    let doc3 = validate_pdf(&out3, 3);
    let pages3 = doc3.get_pages();
    assert_eq!(get_effective_page_rotation(&doc3, pages3[&1]), 0);
    assert_eq!(get_effective_page_rotation(&doc3, pages3[&2]), 180);
    assert_eq!(get_effective_page_rotation(&doc3, pages3[&3]), 270);

    // Out-of-bounds selection (parsed against larger total) -> PageOutOfRange
    let bad_sel = PageSelection::parse("5", 10).expect("parse 5");
    let err = rotate_pages(&src, Some(&bad_sel), 90, &opts1, &NoProgress).expect_err("must fail");
    assert!(matches!(
        err,
        SheafError::PageOutOfRange { page: 5, total: 3 }
    ));
}

#[test]
fn delete_pages_removes_selected_and_rejects_deleting_all() {
    let src = fixtures_dir().join("fixture_12p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");
    let out = temp.path().join("deleted.pdf");

    let sel = PageSelection::parse("2, 5-8, 12", 12).expect("parse");
    let opts = PageOpOptions {
        out: Some(out.clone()),
        force: false,
        dry_run: false,
    };

    let rep = delete_pages(&src, &sel, &opts, &NoProgress).expect("delete pages");
    assert_eq!(rep.action, "delete");
    assert_eq!(rep.input_pages, 12);
    assert_eq!(rep.output_pages, 6);
    assert_eq!(rep.pages_affected, vec![2, 5, 6, 7, 8, 12]);
    validate_pdf(&out, 6);

    // Deleting all pages must return InvalidSelection
    let all_sel = PageSelection::parse("1-12", 12).expect("parse all");
    let err = delete_pages(
        &src,
        &all_sel,
        &PageOpOptions {
            out: Some(temp.path().join("empty.pdf")),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect_err("cannot delete all pages");
    assert!(matches!(err, SheafError::InvalidSelection(_)));
}

#[test]
fn reorder_pages_updates_page_sequence() {
    let src = fixtures_dir().join("fixture_b_3p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");
    let out = temp.path().join("reordered.pdf");

    let order = PageSelection::parse("3, 1, 2", 3).expect("parse");
    let opts = PageOpOptions {
        out: Some(out.clone()),
        force: false,
        dry_run: false,
    };

    let rep = reorder_pages(&src, &order, &opts, &NoProgress).expect("reorder");
    assert_eq!(rep.action, "reorder");
    assert_eq!(rep.input_pages, 3);
    assert_eq!(rep.output_pages, 3);
    assert_eq!(rep.pages_affected, vec![3, 1, 2]);

    let doc = validate_pdf(&out, 3);
    // Verify text order: Page 1 of reordered doc should be original Page 3 ("DocB - Page 3")
    let text1 = doc.extract_text(&[1]).expect("extract text p1");
    let text2 = doc.extract_text(&[2]).expect("extract text p2");
    let text3 = doc.extract_text(&[3]).expect("extract text p3");
    assert!(text1.contains("Page 3"), "got: {text1}");
    assert!(text2.contains("Page 1"), "got: {text2}");
    assert!(text3.contains("Page 2"), "got: {text3}");
}

#[test]
fn extract_pages_creates_valid_subset_pdf() {
    let src = fixtures_dir().join("fixture_12p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");
    let out = temp.path().join("extracted.pdf");

    let sel = PageSelection::parse("4, 9-11", 12).expect("parse");
    let opts = PageOpOptions {
        out: Some(out.clone()),
        force: false,
        dry_run: false,
    };

    let rep = extract_pages(&src, &sel, &opts, &NoProgress).expect("extract");
    assert_eq!(rep.action, "extract");
    assert_eq!(rep.input_pages, 12);
    assert_eq!(rep.output_pages, 4);
    assert_eq!(rep.pages_affected, vec![4, 9, 10, 11]);

    let doc = validate_pdf(&out, 4);
    let text1 = doc.extract_text(&[1]).expect("extract text p1");
    assert!(text1.contains("Page 4"), "got: {text1}");
}
