use std::path::{Path, PathBuf};

use lopdf::{Document, dictionary};
use sheaf_core::{
    LoadOptions, NoProgress, NumberOptions, NumberPosition, RenderOptions, RenderScale, SheafError,
    format_page_number, is_pdfium_available, load_pdf, number, render_with_options,
};
use tempfile::TempDir;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .join("tests")
        .join("fixtures")
}

fn fixture_path(name: &str) -> PathBuf {
    fixtures_dir().join(name)
}

#[test]
fn test_number_placeholders_and_start() {
    let dir = TempDir::new().unwrap();
    let src = fixture_path("fixture_b_3p.pdf");
    let out = dir.path().join("numbered_custom.pdf");

    let opts = NumberOptions {
        format: "Page {n} of {total} (Doc Page {page}/{pages})".to_string(),
        position: NumberPosition::BottomRight,
        start: 10,
        pages: Some("2-3".to_string()),
        out: Some(out.clone()),
        ..NumberOptions::default()
    };

    let report = number(&src, &opts, &NoProgress).expect("number pages");
    assert_eq!(report.pages_numbered, vec![2, 3]);
    assert_eq!(report.total_pages, 3);
    assert_eq!(report.start, 10);
    assert_eq!(report.output, out);

    // Verify document loads cleanly
    let loaded = load_pdf(&out, &LoadOptions::none()).expect("load output");
    assert_eq!(loaded.doc.get_pages().len(), 3);

    // Page 1 should NOT have overlay font
    let p1_id = *loaded.doc.get_pages().get(&1).unwrap();
    let p1_dict = loaded.doc.get_dictionary(p1_id).unwrap();
    if let Ok(res) = p1_dict.get(b"Resources").and_then(|o| o.as_dict()) {
        if let Ok(fonts) = res.get(b"Font").and_then(|o| o.as_dict()) {
            assert!(
                !fonts.has(b"SheafF0"),
                "Page 1 was not selected for numbering"
            );
        }
    }

    // Page 2 and 3 MUST have overlay font
    for p in [2, 3] {
        let p_id = *loaded.doc.get_pages().get(&p).unwrap();
        let p_dict = loaded.doc.get_dictionary(p_id).unwrap();
        let res = p_dict.get(b"Resources").unwrap().as_dict().unwrap();
        let fonts = res.get(b"Font").unwrap().as_dict().unwrap();
        assert!(fonts.has(b"SheafF0"), "Page {p} must have numbering font");
    }
}

#[test]
fn test_number_formatting_logic() {
    // Basic {n}
    assert_eq!(format_page_number("{n}", 1, 1, 1, 1).unwrap(), "1");
    // All placeholders: seq_n, selected_total, page_num, total_pages
    assert_eq!(
        format_page_number("Page {n}/{total} - doc {page}/{pages}", 5, 10, 2, 20).unwrap(),
        "Page 5/10 - doc 2/20"
    );
    // Invalid placeholders
    assert!(format_page_number("{unknown}", 1, 1, 1, 1).is_err());
    assert!(format_page_number("unclosed {n", 1, 1, 1, 1).is_err());
}

#[test]
fn test_number_validation_bounds() {
    let _dir = TempDir::new().unwrap();
    let src = fixture_path("fixture_a_2p.pdf");

    // Font size too small (< 4 pt)
    let opts_too_small = NumberOptions {
        font_size: 3.5,
        ..NumberOptions::default()
    };
    let err_small = number(&src, &opts_too_small, &NoProgress).unwrap_err();
    assert!(matches!(err_small, SheafError::InvalidSelection(_)));

    // Font size too large (> 72 pt)
    let opts_too_large = NumberOptions {
        font_size: 73.0,
        ..NumberOptions::default()
    };
    let err_large = number(&src, &opts_too_large, &NoProgress).unwrap_err();
    assert!(matches!(err_large, SheafError::InvalidSelection(_)));

    // Start must be >= 1
    let opts_zero_start = NumberOptions {
        start: 0,
        ..NumberOptions::default()
    };
    let err_start = number(&src, &opts_zero_start, &NoProgress).unwrap_err();
    assert!(matches!(err_start, SheafError::InvalidSelection(_)));

    // Opacity bounds (0.05..=1.0)
    let opts_low_opacity = NumberOptions {
        opacity: 0.01,
        ..NumberOptions::default()
    };
    assert!(number(&src, &opts_low_opacity, &NoProgress).is_err());

    let opts_high_opacity = NumberOptions {
        opacity: 1.5,
        ..NumberOptions::default()
    };
    assert!(number(&src, &opts_high_opacity, &NoProgress).is_err());
}

#[test]
fn test_number_rotations_and_rendering() {
    if !is_pdfium_available() {
        return;
    }

    let dir = TempDir::new().unwrap();

    for rot in [90, 180, 270] {
        let src = dir.path().join(format!("rotated_{rot}.pdf"));
        // Build temporary rotated PDF
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let page_id = doc.add_object(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Rotate" => rot,
        });
        doc.objects.insert(
            pages_id,
            lopdf::Object::Dictionary(lopdf::dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page_id.into()],
                "Count" => 1,
            }),
        );
        let cat_id = doc.add_object(lopdf::dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", cat_id);
        doc.save(&src).unwrap();

        let out = dir.path().join(format!("num_rot_{rot}.pdf"));
        let opts = NumberOptions {
            format: "Rot {page}".to_string(),
            position: NumberPosition::BottomRight,
            out: Some(out.clone()),
            ..NumberOptions::default()
        };
        let rep = number(&src, &opts, &NoProgress).expect("number rotated");
        assert_eq!(rep.pages_numbered, vec![1]);

        // Render with PDFium to verify it doesn't crash or error on rotated numbered pages
        let rend_dir = dir.path().join(format!("renders_{rot}"));
        let rend_opts = RenderOptions {
            load: LoadOptions::none(),
            pages: Some("1".to_string()),
            scale: RenderScale::WidthPx(200),
            out: rend_dir,
            force: true,
            dry_run: false,
            use_cache: false,
            cache_dir: None,
        };
        let rend_rep = render_with_options(&out, &rend_opts, &NoProgress).expect("render");
        assert_eq!(rend_rep.outputs.len(), 1);
        assert!(rend_rep.outputs[0].path.exists());
    }
}
