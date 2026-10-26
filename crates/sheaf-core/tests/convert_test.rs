use std::path::{Path, PathBuf};
use std::process::Command;

use image::ImageReader;
use lopdf::Document;
use sheaf_core::{
    ConvertPageSize, ImageFormat, ImagesToPdfOptions, LoadOptions, MarkdownToPdfOptions,
    NoProgress, PdfToImagesOptions, PdfToMarkdownOptions, Progress, SheafError, images_to_pdf,
    load_pdf, markdown_to_pdf, pdf_to_images, pdf_to_markdown,
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
fn roundtrip_pdf_to_images_and_images_to_pdf() {
    let src_pdf = fixtures_dir().join("fixture_b_3p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    // 1. Export all 3 pages to PNG at 150 DPI
    let png_dir = temp.path().join("png_pages");
    let png_rep = pdf_to_images(
        &src_pdf,
        &PdfToImagesOptions {
            load: LoadOptions::none(),
            format: ImageFormat::Png,
            dpi: 150,
            quality: 85,
            pages: None,
            out_dir: Some(png_dir.clone()),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect("pdf_to_images png");

    assert_eq!(png_rep.total_pages, 3);
    assert_eq!(png_rep.outputs.len(), 3);
    for item in &png_rep.outputs {
        assert!(item.path.exists());
        let decoded = ImageReader::open(&item.path)
            .expect("open png")
            .decode()
            .expect("decode png");
        assert_eq!(decoded.width(), item.width_px);
        assert_eq!(decoded.height(), item.height_px);
    }

    // 2. Export pages 1,3 to JPG at 72 DPI
    let jpg_dir = temp.path().join("jpg_pages");
    let jpg_rep = pdf_to_images(
        &src_pdf,
        &PdfToImagesOptions {
            load: LoadOptions::none(),
            format: ImageFormat::Jpg,
            dpi: 72,
            quality: 80,
            pages: Some("1,3".to_string()),
            out_dir: Some(jpg_dir),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect("pdf_to_images jpg");
    assert_eq!(jpg_rep.outputs.len(), 2);

    // 3. Recombine the 3 PNG images into a PDF (Fit and A4 with margin)
    let image_paths: Vec<PathBuf> = png_rep.outputs.iter().map(|o| o.path.clone()).collect();
    let fit_pdf = temp.path().join("recombined_fit.pdf");
    let fit_rep = images_to_pdf(
        &image_paths,
        &ImagesToPdfOptions {
            page_size: ConvertPageSize::Fit,
            margin_pt: 0.0,
            out: fit_pdf.clone(),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect("images_to_pdf fit");
    assert_eq!(fit_rep.pages, 3);
    validate_pdf(&fit_pdf, 3);

    let a4_pdf = temp.path().join("recombined_a4.pdf");
    let a4_rep = images_to_pdf(
        &image_paths,
        &ImagesToPdfOptions {
            page_size: ConvertPageSize::A4,
            margin_pt: 36.0,
            out: a4_pdf.clone(),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect("images_to_pdf a4");
    assert_eq!(a4_rep.pages, 3);
    validate_pdf(&a4_pdf, 3);
}

#[test]
fn roundtrip_markdown_to_pdf_and_pdf_to_markdown() {
    let temp = tempfile::tempdir().expect("tempdir");
    let md_path = temp.path().join("architecture.md");

    let sample_md = "\
# Sheaf Architecture Guide

Sheaf is a fast local PDF toolkit for macOS written in Rust.

## Core Principles

- Zero network access and zero telemetry
- Atomic file writes via temporary files
- Deterministic JSON output contract

1. Parse and validate inputs
2. Execute on worker thread

> Original input files are never modified in place.

---

### CLI Example

```bash
sheaf convert from-markdown architecture.md -o architecture.pdf
```
";
    std::fs::write(&md_path, sample_md).expect("write sample markdown");

    // 1. Render Markdown -> PDF
    let pdf_out = temp.path().join("architecture.pdf");
    let md_to_pdf_rep = markdown_to_pdf(
        &md_path,
        &MarkdownToPdfOptions {
            page_size: ConvertPageSize::A4,
            margin_pt: 54.0,
            out: Some(pdf_out.clone()),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect("markdown_to_pdf");

    assert!(md_to_pdf_rep.pages >= 1);
    assert_eq!(md_to_pdf_rep.headings_count, 3);
    let doc = validate_pdf(&pdf_out, md_to_pdf_rep.pages as usize);
    assert!(
        doc.catalog().unwrap().get(b"Outlines").is_ok(),
        "expected PDF bookmarks/outlines generated from Markdown headings"
    );

    // 2. Extract PDF -> Markdown
    let extracted_md_path = temp.path().join("extracted.md");
    let pdf_to_md_rep = pdf_to_markdown(
        &pdf_out,
        &PdfToMarkdownOptions {
            load: LoadOptions::none(),
            pages: None,
            out: Some(extracted_md_path.clone()),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect("pdf_to_markdown");

    assert_eq!(pdf_to_md_rep.extracted_pages, md_to_pdf_rep.pages);
    assert!(pdf_to_md_rep.char_count > 100);

    let extracted_md = std::fs::read_to_string(&extracted_md_path).expect("read extracted.md");
    assert!(extracted_md.contains("# Sheaf Architecture Guide"));
    assert!(extracted_md.contains("## Core Principles"));
    assert!(extracted_md.contains("### CLI Example"));
    assert!(extracted_md.contains("- Zero network access and zero telemetry"));
    assert!(extracted_md.contains("sheaf convert from-markdown"));
}

#[test]
fn convert_dry_run_and_error_cases() {
    let src_pdf = fixtures_dir().join("fixture_b_3p.pdf");
    let encrypted = fixtures_dir().join("encrypted.pdf");
    let temp = tempfile::tempdir().expect("tempdir");

    // Dry-run pdf_to_images writes nothing
    let dry_dir = temp.path().join("dry_imgs");
    let dry_rep = pdf_to_images(
        &src_pdf,
        &PdfToImagesOptions {
            out_dir: Some(dry_dir.clone()),
            dry_run: true,
            ..Default::default()
        },
        &NoProgress,
    )
    .expect("dry run pdf_to_images");
    assert_eq!(dry_rep.outputs.len(), 3);
    assert!(!dry_dir.exists());

    // EmptySelection on images_to_pdf with no inputs
    let empty_err = images_to_pdf::<PathBuf>(
        &[],
        &ImagesToPdfOptions {
            page_size: ConvertPageSize::Fit,
            margin_pt: 0.0,
            out: temp.path().join("empty.pdf"),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect_err("empty inputs");
    assert!(matches!(empty_err, SheafError::EmptySelection));

    // Encrypted PDF in pdf_to_markdown
    let enc_err = pdf_to_markdown(
        &encrypted,
        &PdfToMarkdownOptions {
            load: LoadOptions::none(),
            pages: None,
            out: Some(temp.path().join("enc.md")),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect_err("encrypted pdf_to_markdown");
    assert!(matches!(enc_err, SheafError::Encrypted));

    // Cancelled in pdf_to_markdown
    let cancel_err = pdf_to_markdown(
        &src_pdf,
        &PdfToMarkdownOptions {
            load: LoadOptions::none(),
            pages: None,
            out: Some(temp.path().join("cancel.md")),
            force: false,
            dry_run: false,
        },
        &CancelledProgress,
    )
    .expect_err("cancelled");
    assert!(matches!(cancel_err, SheafError::Cancelled));
}
