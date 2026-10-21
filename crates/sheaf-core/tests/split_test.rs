use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;

use lopdf::content::{Content, Operation};
use lopdf::{Bookmark, Document, Object, Stream, dictionary};
use proptest::prelude::*;
use sheaf_core::fs::write_atomic;
use sheaf_core::{
    LoadOptions, NoProgress, PageSelection, SheafError, SplitMode, SplitOptions, split,
};

static INIT_SPLIT_FIXTURES: Once = Once::new();

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .join("tests")
        .join("fixtures")
}

fn build_pdf_with_bookmarks(
    prefix: &str,
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
        "Font" => dictionary! { "F1" => font_id },
    });

    let mut page_ids = Vec::with_capacity(page_count as usize);
    let mut kids = Vec::with_capacity(page_count as usize);

    for page_idx in 1..=page_count {
        let label = format!("{prefix} - Page {page_idx}");
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

fn ensure_split_fixtures() -> PathBuf {
    let dir = fixtures_dir();
    INIT_SPLIT_FIXTURES.call_once(|| {
        std::fs::create_dir_all(&dir).expect("create fixtures dir");

        // 1. fixture_12p.pdf (12 pages)
        let mut doc_12 = build_pdf_with_bookmarks("Report12", 12, &[]);
        doc_12
            .save(dir.join("fixture_12p.pdf"))
            .expect("save fixture_12p.pdf");

        // 2. fixture_outlines.pdf (12 pages, 3 chapters on level 1, each with 2 subchapters on level 2, plus page 1 front matter)
        // Page 1: Front matter (before first bookmark)
        // Chapter 1 (p2): Sub 1.1 (p2), Sub 1.2 (p4)
        // Chapter 2 (p6): Sub 2.1 (p6), Sub 2.2 (p8)
        // Chapter 3 (p10): Sub 3.1 (p10), Sub 3.2 (p11)
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
        let mut doc_outlines = build_pdf_with_bookmarks("Book", 12, &outlines);
        doc_outlines
            .save(dir.join("fixture_outlines.pdf"))
            .expect("save fixture_outlines.pdf");

        // 3. fixture_no_outlines.pdf (4 pages, no outlines)
        let mut doc_no = build_pdf_with_bookmarks("Plain", 4, &[]);
        doc_no
            .save(dir.join("fixture_no_outlines.pdf"))
            .expect("save fixture_no_outlines.pdf");
    });
    dir
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

// --- 1. Unit tests for PageSelection::parse (all table cases from Section 6) ---

#[test]
fn page_selection_parse_valid_cases() {
    let s1 = PageSelection::parse("3", 12).expect("single page");
    assert_eq!(s1.ranges(), std::slice::from_ref(&(3..=3)));

    let s2 = PageSelection::parse("1-3", 12).expect("closed range");
    assert_eq!(s2.ranges(), std::slice::from_ref(&(1..=3)));

    let s3 = PageSelection::parse("8-", 12).expect("open end range");
    assert_eq!(s3.ranges(), std::slice::from_ref(&(8..=12)));

    let s4 = PageSelection::parse("-3", 12).expect("open start range");
    assert_eq!(s4.ranges(), std::slice::from_ref(&(1..=3)));

    let s5 = PageSelection::parse(" 1-3 , 5 , 8- , 2-4 ", 12)
        .expect("multi token with overlap and spaces");
    assert_eq!(s5.ranges(), &[1..=3, 5..=5, 8..=12, 2..=4]);
}

#[test]
fn page_selection_parse_error_cases() {
    // Empty string -> EmptySelection
    assert!(matches!(
        PageSelection::parse("", 12),
        Err(SheafError::EmptySelection)
    ));
    assert!(matches!(
        PageSelection::parse("   ", 12),
        Err(SheafError::EmptySelection)
    ));

    // Page 0 or > total -> PageOutOfRange
    assert!(matches!(
        PageSelection::parse("0", 12),
        Err(SheafError::PageOutOfRange { page: 0, total: 12 })
    ));
    assert!(matches!(
        PageSelection::parse("13", 12),
        Err(SheafError::PageOutOfRange {
            page: 13,
            total: 12
        })
    ));
    assert!(matches!(
        PageSelection::parse("1-15", 12),
        Err(SheafError::PageOutOfRange {
            page: 15,
            total: 12
        })
    ));

    // Reversed range 5-3 -> InvalidSelection
    assert!(matches!(
        PageSelection::parse("5-3", 12),
        Err(SheafError::InvalidSelection(_))
    ));

    // Malformed syntax -> InvalidSelection
    for bad in ["abc", "1--3", ",,", "1,", ",2", "-", "1-2-3"] {
        assert!(
            matches!(
                PageSelection::parse(bad, 12),
                Err(SheafError::InvalidSelection(_))
            ),
            "Expected InvalidSelection for '{bad}'"
        );
    }
}

// --- 2. Property tests (proptest) ---

proptest! {
    #[test]
    fn proptest_page_selection_never_panics(input in "\\PC*", total in 0u32..1000u32) {
        if let Ok(sel) = PageSelection::parse(&input, total) {
            prop_assert!(!sel.ranges().is_empty());
            for r in sel.ranges() {
                prop_assert!(*r.start() >= 1);
                prop_assert!(*r.end() <= total);
                prop_assert!(r.start() <= r.end());
            }
        }
    }
}

// --- 3. Core integration tests for Split ---

#[test]
fn split_by_ranges_separate_and_combine() {
    let fixtures = ensure_split_fixtures();
    let src = fixtures.join("fixture_12p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    // 3a. Separate files (with overlapping ranges + duplicate range)
    let out_sep = temp.path().join("sep");
    let sel = PageSelection::parse("1-3, 5, 8-, 2-4", 12).expect("parse");
    let opts = SplitOptions {
        load: LoadOptions::none(),
        mode: SplitMode::Ranges {
            selection: sel,
            combine: false,
        },
        out_dir: out_sep,
        force: false,
        dry_run: false,
    };

    let report = split(&src, &opts, &NoProgress).expect("split ranges");
    assert_eq!(report.input_pages, 12);
    assert_eq!(report.mode, "ranges");
    assert_eq!(report.outputs.len(), 4);

    let expected = [
        ("fixture_12p_p1-3.pdf", 3usize, Some((1, 3))),
        ("fixture_12p_p5.pdf", 1usize, Some((5, 5))),
        ("fixture_12p_p8-12.pdf", 5usize, Some((8, 12))),
        ("fixture_12p_p2-4.pdf", 3usize, Some((2, 4))),
    ];

    for (out_item, (name, pages, range)) in report.outputs.iter().zip(expected) {
        assert_eq!(out_item.path.file_name().unwrap().to_str().unwrap(), name);
        assert_eq!(out_item.pages as usize, pages);
        assert_eq!(out_item.page_range, range);
        let doc = validate_pdf(&out_item.path, pages);
        // Metadata (Info dict) preserved
        assert!(doc.trailer.get(b"Info").is_ok());
    }

    // 3b. Combined into single file
    let out_comb = temp.path().join("comb");
    let sel_comb = PageSelection::parse("1-3, 5, 2-3", 12).expect("parse");
    let opts_comb = SplitOptions {
        load: LoadOptions::none(),
        mode: SplitMode::Ranges {
            selection: sel_comb,
            combine: true,
        },
        out_dir: out_comb,
        force: false,
        dry_run: false,
    };

    let report_comb = split(&src, &opts_comb, &NoProgress).expect("split combine");
    assert_eq!(report_comb.outputs.len(), 1);
    assert_eq!(
        report_comb.outputs[0]
            .path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap(),
        "fixture_12p_selection.pdf"
    );
    assert_eq!(report_comb.outputs[0].pages, 6);
    validate_pdf(&report_comb.outputs[0].path, 6);
}

#[test]
fn split_every_n_sum_equals_original() {
    let fixtures = ensure_split_fixtures();
    let src = fixtures.join("fixture_12p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    let opts = SplitOptions {
        load: LoadOptions::none(),
        mode: SplitMode::EveryN(5),
        out_dir: temp.path().to_path_buf(),
        force: false,
        dry_run: false,
    };

    let report = split(&src, &opts, &NoProgress).expect("split every 5");
    assert_eq!(report.mode, "every_n");
    assert_eq!(report.outputs.len(), 3);

    let mut page_sum = 0u32;
    let expected_pages = [5u32, 5, 2];
    for (idx, (item, &exp_p)) in report.outputs.iter().zip(&expected_pages).enumerate() {
        let exp_name = format!("fixture_12p_part{:02}.pdf", idx + 1);
        assert_eq!(item.path.file_name().unwrap().to_str().unwrap(), exp_name);
        assert_eq!(item.pages, exp_p);
        validate_pdf(&item.path, exp_p as usize);
        page_sum += item.pages;
    }
    assert_eq!(page_sum, report.input_pages);
}

#[test]
fn split_by_bookmarks_level_1_and_level_2() {
    let fixtures = ensure_split_fixtures();
    let src = fixtures.join("fixture_outlines.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    // Level 1: front (p1) + 3 chapters (p2-5, p6-9, p10-12)
    let out_l1 = temp.path().join("l1");
    let opts_l1 = SplitOptions {
        load: LoadOptions::none(),
        mode: SplitMode::ByBookmarks { level: 1 },
        out_dir: out_l1,
        force: false,
        dry_run: false,
    };
    let report_l1 = split(&src, &opts_l1, &NoProgress).expect("split bookmarks level 1");
    assert_eq!(report_l1.outputs.len(), 4);

    let expected_l1 = [
        ("fixture_outlines_00_front.pdf", 1usize, (1, 1)),
        ("fixture_outlines_01_1-einfuehrung.pdf", 4usize, (2, 5)),
        ("fixture_outlines_02_2-hauptteil.pdf", 4usize, (6, 9)),
        ("fixture_outlines_03_3-ausblick-fazit.pdf", 3usize, (10, 12)),
    ];

    for (item, (name, pages, range)) in report_l1.outputs.iter().zip(expected_l1) {
        assert_eq!(item.path.file_name().unwrap().to_str().unwrap(), name);
        assert_eq!(item.pages as usize, pages);
        assert_eq!(item.page_range, Some(range));
        let doc = validate_pdf(&item.path, pages);
        // Outlines removed in split output
        assert!(doc.get_toc().is_err());
    }

    // Level 2: front (p1) + 6 subchapters (p2-3, p4-5, p6-7, p8-9, p10-10, p11-12)
    let out_l2 = temp.path().join("l2");
    let opts_l2 = SplitOptions {
        load: LoadOptions::none(),
        mode: SplitMode::ByBookmarks { level: 2 },
        out_dir: out_l2,
        force: false,
        dry_run: false,
    };
    let report_l2 = split(&src, &opts_l2, &NoProgress).expect("split bookmarks level 2");
    assert_eq!(report_l2.outputs.len(), 7);
    let total_l2: u32 = report_l2.outputs.iter().map(|o| o.pages).sum();
    assert_eq!(total_l2, 12);

    // No outlines on document -> Unsupported
    let src_no = fixtures.join("fixture_no_outlines.pdf");
    let err = split(&src_no, &opts_l1, &NoProgress).expect_err("no outlines must fail");
    assert!(matches!(err, SheafError::Unsupported(_)));
}

// --- 4. Atomic write & Error mapping tests ---

#[test]
fn write_atomic_leaves_no_partial_file_on_error() {
    let temp = tempfile::tempdir().expect("tempdir");
    let target = temp.path().join("should_not_exist.pdf");

    let res = write_atomic(&target, false, |file| {
        file.write_all(b"%PDF-1.5 partial content")?;
        Err(SheafError::Cancelled)
    });

    assert!(matches!(res, Err(SheafError::Cancelled)));
    assert!(
        !target.exists(),
        "Target file must not exist after failed write"
    );

    // Ensure no temporary file was left behind in the directory
    let entries: Vec<_> = std::fs::read_dir(temp.path()).expect("read_dir").collect();
    assert!(
        entries.is_empty(),
        "Temp files must be cleaned up on failure"
    );
}

#[test]
fn sheaf_error_codes_and_hints_are_exhaustive() {
    let samples = [
        (
            SheafError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "missing")),
            "io",
        ),
        (SheafError::InvalidPdf("bad".to_string()), "invalid_pdf"),
        (SheafError::Encrypted, "encrypted"),
        (SheafError::WrongPassword, "wrong_password"),
        (
            SheafError::PageOutOfRange {
                page: 99,
                total: 12,
            },
            "page_out_of_range",
        ),
        (SheafError::EmptySelection, "empty_selection"),
        (
            SheafError::InvalidSelection("5-3".to_string()),
            "invalid_selection",
        ),
        (
            SheafError::OutputExists(PathBuf::from("out.pdf")),
            "output_exists",
        ),
        (
            SheafError::Unsupported("keine Lesezeichen".to_string()),
            "unsupported",
        ),
        (SheafError::Cancelled, "cancelled"),
    ];

    for (err, expected_code) in samples {
        assert_eq!(err.code(), expected_code);
        if !matches!(err, SheafError::Cancelled) {
            assert!(err.hint().is_some(), "Expected hint for {expected_code}");
        }
    }
}
