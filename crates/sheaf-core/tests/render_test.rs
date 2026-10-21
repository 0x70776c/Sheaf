use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::Instant;

use image::ImageReader;
use lopdf::{
    Dictionary, Document, Object, Stream, content::Content, content::Operation, dictionary,
};
use sheaf_core::{
    DEFAULT_THUMBNAIL_WIDTH_PX, LoadOptions, NoProgress, RenderOptions, RenderScale, SheafError,
    render_page, render_thumbnails, render_with_options, thumbnail_cache_key,
};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .join("tests")
        .join("fixtures")
}

/// Ensures `tests/fixtures/fixture_500p.pdf` exists and has 500 valid pages.
fn ensure_500p_fixture() -> PathBuf {
    let path = fixtures_dir().join("fixture_500p.pdf");
    if path.exists()
        && let Ok(doc) = Document::load(&path)
        && doc.get_pages().len() == 500
    {
        return path;
    }

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

    // Share a compact content stream so the 500-page fixture stays small and fast
    let content = Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 24.into()]),
            Operation::new("Td", vec![72.into(), 720.into()]),
            Operation::new("Tj", vec![Object::string_literal("Sheaf 500p Benchmark")]),
            Operation::new("ET", vec![]),
        ],
    };
    let content_id = doc.add_object(Stream::new(
        dictionary! {},
        content.encode().expect("encode content"),
    ));

    let mut kids = Vec::with_capacity(500);
    for _ in 0..500 {
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
        });
        kids.push(page_id.into());
    }

    let pages_dict = Dictionary::from_iter(vec![
        ("Type", "Pages".into()),
        ("Kids", Object::Array(kids)),
        ("Count", 500.into()),
        (
            "MediaBox",
            vec![0.into(), 0.into(), 595.into(), 842.into()].into(),
        ),
        ("Resources", resources_id.into()),
    ]);
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    doc.compress();
    doc.save(&path).expect("save fixture_500p.pdf");
    path
}

fn decode_png_dimensions(png_bytes: &[u8]) -> (u32, u32) {
    assert!(
        png_bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "Output does not start with PNG signature"
    );
    let img = ImageReader::new(Cursor::new(png_bytes))
        .with_guessed_format()
        .expect("guess image format")
        .decode()
        .expect("decode PNG bytes");
    (img.width(), img.height())
}

#[test]
fn render_page_produces_valid_png_with_expected_aspect_ratio() {
    let src = fixtures_dir().join("fixture_b_3p.pdf");

    let png_160 = render_page(&src, 1, 160).expect("render page 1 at 160px");
    let (w1, h1) = decode_png_dimensions(&png_160);
    assert_eq!(w1, 160);
    // Standard portrait page (595x842 or 612x792) has aspect ratio ~1.29 to 1.42
    assert!(
        (200..=240).contains(&h1),
        "Unexpected height {h1} for width 160"
    );

    let png_320 = render_page(&src, 2, 320).expect("render page 2 at 320px");
    let (w2, h2) = decode_png_dimensions(&png_320);
    assert_eq!(w2, 320);
    assert!((h2 as i32 - (h1 as i32 * 2)).abs() <= 2);
}

#[test]
fn render_500p_first_20_thumbnails_under_one_second_and_caches() {
    let src = ensure_500p_fixture();
    let cache_dir = tempfile::tempdir().expect("temp cache dir");
    let pages: Vec<u32> = (1..=20).collect();

    // Warm up PDFium library binding once before timing document load + 20 thumbnail renders
    let _ = sheaf_core::is_pdfium_available();

    let start_cold = Instant::now();
    let pngs_cold = render_thumbnails(
        &src,
        &pages,
        DEFAULT_THUMBNAIL_WIDTH_PX,
        Some(cache_dir.path()),
        &NoProgress,
    )
    .expect("cold render 20 thumbnails from 500p PDF");
    let elapsed_cold = start_cold.elapsed();

    assert_eq!(pngs_cold.len(), 20);
    for png in &pngs_cold {
        let (w, h) = decode_png_dimensions(png);
        assert_eq!(w, DEFAULT_THUMBNAIL_WIDTH_PX);
        assert!(h > 200);
    }
    assert!(
        elapsed_cold.as_secs_f64() < 1.0,
        "Expected 20 thumbnails from 500p PDF in < 1.0s, took {:.3}s",
        elapsed_cold.as_secs_f64()
    );

    // Second run should hit the disk cache and return identical bytes
    let start_warm = Instant::now();
    let pngs_warm = render_thumbnails(
        &src,
        &pages,
        DEFAULT_THUMBNAIL_WIDTH_PX,
        Some(cache_dir.path()),
        &NoProgress,
    )
    .expect("warm render 20 thumbnails from cache");
    let elapsed_warm = start_warm.elapsed();

    assert_eq!(pngs_cold, pngs_warm);
    assert!(
        elapsed_warm.as_secs_f64() < 0.2,
        "Expected cached thumbnails in < 0.2s, took {:.3}s",
        elapsed_warm.as_secs_f64()
    );

    let key1 = thumbnail_cache_key(&src, 1, DEFAULT_THUMBNAIL_WIDTH_PX).expect("cache key");
    assert!(cache_dir.path().join(format!("{key1}.png")).exists());
}

#[test]
fn render_with_options_supports_dpi_width_dry_run_and_force() {
    let src = fixtures_dir().join("fixture_b_3p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");
    let cache_dir = temp.path().join("cache");

    // 1. Dry-run with DPI
    let out_dir = temp.path().join("out_dpi");
    let dry_opts = RenderOptions {
        load: LoadOptions::none(),
        pages: Some("1-2".to_string()),
        scale: RenderScale::Dpi(150),
        out: out_dir.clone(),
        force: false,
        dry_run: true,
        use_cache: false,
        cache_dir: None,
    };
    let dry_rep = render_with_options(&src, &dry_opts, &NoProgress).expect("dry run");
    assert!(dry_rep.dry_run);
    assert_eq!(dry_rep.total_pages, 3);
    assert_eq!(dry_rep.outputs.len(), 2);
    assert!(!dry_rep.outputs[0].path.exists());

    // 2. Real render of single page to explicit .png path
    let single_png = temp.path().join("page1.png");
    let single_opts = RenderOptions {
        load: LoadOptions::none(),
        pages: Some("1".to_string()),
        scale: RenderScale::WidthPx(160),
        out: single_png.clone(),
        force: false,
        dry_run: false,
        use_cache: true,
        cache_dir: Some(cache_dir.clone()),
    };
    let rep1 = render_with_options(&src, &single_opts, &NoProgress).expect("render single");
    assert_eq!(rep1.outputs.len(), 1);
    assert_eq!(rep1.outputs[0].path, single_png);
    assert_eq!(rep1.outputs[0].width_px, 160);
    assert!(!rep1.outputs[0].cached);
    assert!(single_png.exists());

    // 3. Re-running without --force fails with OutputExists
    let err_exists = render_with_options(&src, &single_opts, &NoProgress).unwrap_err();
    assert!(matches!(err_exists, SheafError::OutputExists(_)));

    // 4. Re-running with force=true succeeds and hits disk cache
    let force_opts = RenderOptions {
        force: true,
        ..single_opts
    };
    let rep2 = render_with_options(&src, &force_opts, &NoProgress).expect("render force cached");
    assert!(rep2.outputs[0].cached);
}

#[test]
fn render_error_cases() {
    let fixtures = fixtures_dir();
    let src_3p = fixtures.join("fixture_b_3p.pdf");
    let encrypted = fixtures.join("encrypted.pdf");
    let corrupt = fixtures.join("corrupted.pdf");
    let missing = fixtures.join("does_not_exist.pdf");

    assert!(matches!(
        render_page(&src_3p, 0, 160),
        Err(SheafError::PageOutOfRange { page: 0, total: 3 })
    ));
    assert!(matches!(
        render_page(&src_3p, 4, 160),
        Err(SheafError::PageOutOfRange { page: 4, total: 3 })
    ));
    assert!(matches!(
        render_page(&src_3p, 1, 0),
        Err(SheafError::InvalidSelection(_))
    ));
    assert!(matches!(
        render_page(&encrypted, 1, 160),
        Err(SheafError::Encrypted)
    ));
    assert!(matches!(
        render_page(&corrupt, 1, 160),
        Err(SheafError::InvalidPdf(_))
    ));
    assert!(matches!(
        render_page(&missing, 1, 160),
        Err(SheafError::Io(_))
    ));
}
