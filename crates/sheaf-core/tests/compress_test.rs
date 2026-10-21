use std::path::{Path, PathBuf};
use std::process::Command;

use image::codecs::jpeg::JpegEncoder;
use image::{ColorType, RgbImage};
use lopdf::{
    Document, Object, Stream,
    content::{Content, Operation},
    dictionary,
};
use sheaf_core::{
    CompressOptions, CompressPreset, LoadOptions, NoProgress, Progress, SheafError, compress,
    compress_with_options, load_pdf,
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

/// Ensures `tests/fixtures/fixture_photo.pdf` exists with selectable text and
/// high-resolution (400 DPI) RGB photo XObjects across 2 pages.
fn ensure_photo_fixture() -> PathBuf {
    let path = fixtures_dir().join("fixture_photo.pdf");
    if path.exists()
        && let Ok(loaded) = load_pdf(&path, &LoadOptions::none())
        && loaded.doc.get_pages().len() == 2
    {
        return path;
    }

    // Generate a 2200x1650 RGB image (~550 DPI when placed in a 288x216 pt box)
    let width = 2200u32;
    let height = 1650u32;
    let mut img = RgbImage::new(width, height);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let r = ((x * 255) / width) as u8;
        let g = ((y * 255) / height) as u8;
        let b = (((x ^ y) * 17) & 0xFF) as u8;
        *pixel = image::Rgb([r, g, b]);
    }

    let mut jpeg_bytes = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_bytes, 95);
    encoder
        .encode(img.as_raw(), width, height, ColorType::Rgb8.into())
        .expect("encode high-res JPEG");

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    // Two identical high-res image streams so Medium/High also test deduplication
    let img_stream_1 = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => i64::from(width),
            "Height" => i64::from(height),
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8,
            "Filter" => "DCTDecode",
        },
        jpeg_bytes.clone(),
    );
    let img_stream_2 = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => i64::from(width),
            "Height" => i64::from(height),
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8,
            "Filter" => "DCTDecode",
        },
        jpeg_bytes,
    );

    let im1_id = doc.add_object(img_stream_1);
    let im2_id = doc.add_object(img_stream_2);

    // Page 1: Selectable text + Im1 rendered at 288x216 pt
    let content1 = Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 18.into()]),
            Operation::new("Td", vec![72.into(), 720.into()]),
            Operation::new(
                "Tj",
                vec![Object::string_literal("Sheaf Photo Compression Benchmark")],
            ),
            Operation::new("ET", vec![]),
            Operation::new("q", vec![]),
            Operation::new(
                "cm",
                vec![
                    288.into(),
                    0.into(),
                    0.into(),
                    216.into(),
                    72.into(),
                    420.into(),
                ],
            ),
            Operation::new("Do", vec!["Im1".into()]),
            Operation::new("Q", vec![]),
        ],
    };
    let content1_id = doc.add_object(Stream::new(
        dictionary! {},
        content1.encode().expect("encode page 1"),
    ));
    let res1_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
        "XObject" => dictionary! { "Im1" => im1_id },
    });
    let page1_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => res1_id,
        "Contents" => content1_id,
    });

    // Page 2: Selectable text + duplicate image stream Im2
    let content2 = Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 18.into()]),
            Operation::new("Td", vec![72.into(), 720.into()]),
            Operation::new(
                "Tj",
                vec![Object::string_literal("Page Two Selectable Text")],
            ),
            Operation::new("ET", vec![]),
            Operation::new("q", vec![]),
            Operation::new(
                "cm",
                vec![
                    288.into(),
                    0.into(),
                    0.into(),
                    216.into(),
                    72.into(),
                    420.into(),
                ],
            ),
            Operation::new("Do", vec!["Im2".into()]),
            Operation::new("Q", vec![]),
        ],
    };
    let content2_id = doc.add_object(Stream::new(
        dictionary! {},
        content2.encode().expect("encode page 2"),
    ));
    let res2_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
        "XObject" => dictionary! { "Im2" => im2_id },
    });
    let page2_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => res2_id,
        "Contents" => content2_id,
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page1_id.into(), page2_id.into()],
            "Count" => 2,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    doc.save(&path).expect("save fixture_photo.pdf");
    path
}

#[test]
fn compress_medium_saves_over_40_percent_and_preserves_selectable_text() {
    let photo_pdf = ensure_photo_fixture();
    let temp = tempfile::tempdir().expect("tempdir");
    let out = temp.path().join("compressed_medium.pdf");

    let report =
        compress(&photo_pdf, CompressPreset::Medium, &out, &NoProgress).expect("compress medium");

    assert!(
        report.saved_percent > 40.0,
        "expected > 40% reduction on photo PDF with Medium preset, got {:.2}% ({} -> {} bytes)",
        report.saved_percent,
        report.size_before_bytes,
        report.size_after_bytes
    );
    assert!(report.images_resampled >= 1);
    assert!(report.objects_deduped >= 1);
    assert!(!report.already_optimal);

    let doc = validate_pdf(&out, 2);
    let text_p1 = doc.extract_text(&[1]).expect("extract text page 1");
    assert!(
        text_p1.contains("Sheaf Photo Compression Benchmark"),
        "expected selectable text on page 1, got: {text_p1:?}"
    );
    let text_p2 = doc.extract_text(&[2]).expect("extract text page 2");
    assert!(
        text_p2.contains("Page Two Selectable Text"),
        "expected selectable text on page 2, got: {text_p2:?}"
    );
}

#[test]
fn compress_presets_order_and_grayscale_conversion() {
    let photo_pdf = ensure_photo_fixture();
    let temp = tempfile::tempdir().expect("tempdir");

    let out_low = temp.path().join("low.pdf");
    let rep_low =
        compress(&photo_pdf, CompressPreset::Low, &out_low, &NoProgress).expect("compress low");

    let out_med = temp.path().join("med.pdf");
    let rep_med = compress(&photo_pdf, CompressPreset::Medium, &out_med, &NoProgress)
        .expect("compress medium");

    let out_high = temp.path().join("high.pdf");
    let rep_high = compress_with_options(
        &photo_pdf,
        &CompressOptions {
            load: LoadOptions::none(),
            preset: CompressPreset::High,
            grayscale: true,
            out: Some(out_high.clone()),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect("compress high grayscale");

    assert!(rep_low.size_after_bytes < rep_low.size_before_bytes);
    assert!(rep_med.size_after_bytes < rep_low.size_after_bytes);
    assert!(rep_high.size_after_bytes < rep_med.size_after_bytes);

    validate_pdf(&out_low, 2);
    validate_pdf(&out_med, 2);
    let high_doc = validate_pdf(&out_high, 2);

    // Verify grayscale conversion changed Image XObject ColorSpace to DeviceGray
    let has_gray_image = high_doc.objects.values().any(|obj| {
        if let Object::Stream(stream) = obj
            && let Ok(sub) = stream.dict.get(b"Subtype").and_then(Object::as_name)
            && sub == b"Image"
            && let Ok(cs) = stream.dict.get(b"ColorSpace").and_then(Object::as_name)
        {
            cs == b"DeviceGray"
        } else {
            false
        }
    });
    assert!(
        has_gray_image,
        "expected DeviceGray image XObject in high grayscale output"
    );
}

#[test]
fn compress_already_optimal_keeps_original_size() {
    let tiny_pdf = fixtures_dir().join("fixture_c_1p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");
    // Compress once to get an already-compact file
    let first_out = temp.path().join("first.pdf");
    let _ =
        compress(&tiny_pdf, CompressPreset::Medium, &first_out, &NoProgress).expect("first pass");

    // Second pass on the already-compressed text-only PDF should not grow the file
    let second_out = temp.path().join("second.pdf");
    let report =
        compress(&first_out, CompressPreset::Low, &second_out, &NoProgress).expect("second pass");

    assert!(report.size_after_bytes <= report.size_before_bytes);
    if report.already_optimal {
        assert_eq!(report.saved_bytes, 0);
        assert_eq!(report.saved_percent, 0.0);
        assert_eq!(report.size_after_bytes, report.size_before_bytes);
    }
    validate_pdf(&second_out, 1);
}

#[test]
fn compress_dry_run_and_error_cases() {
    let photo_pdf = ensure_photo_fixture();
    let temp = tempfile::tempdir().expect("tempdir");
    let dry_out = temp.path().join("dry.pdf");

    let dry_rep = compress_with_options(
        &photo_pdf,
        &CompressOptions {
            load: LoadOptions::none(),
            preset: CompressPreset::Medium,
            grayscale: false,
            out: Some(dry_out.clone()),
            force: false,
            dry_run: true,
        },
        &NoProgress,
    )
    .expect("dry run");
    assert!(dry_rep.saved_percent > 40.0);
    assert!(!dry_out.exists(), "dry_run must not write output file");

    // OutputExists without force
    let existing = temp.path().join("existing.pdf");
    std::fs::write(&existing, b"placeholder").expect("write existing");
    let err = compress_with_options(
        &photo_pdf,
        &CompressOptions {
            load: LoadOptions::none(),
            preset: CompressPreset::Medium,
            grayscale: false,
            out: Some(existing),
            force: false,
            dry_run: false,
        },
        &NoProgress,
    )
    .expect_err("should fail with OutputExists");
    assert!(matches!(err, SheafError::OutputExists(_)));

    // Encrypted PDF
    let encrypted = fixtures_dir().join("encrypted.pdf");
    let enc_out = temp.path().join("enc_out.pdf");
    let enc_err = compress(&encrypted, CompressPreset::Medium, &enc_out, &NoProgress)
        .expect_err("should fail with Encrypted");
    assert!(matches!(enc_err, SheafError::Encrypted));

    // Corrupted PDF
    let corrupted = fixtures_dir().join("corrupted.pdf");
    let cor_out = temp.path().join("cor_out.pdf");
    let cor_err = compress(&corrupted, CompressPreset::Medium, &cor_out, &NoProgress)
        .expect_err("should fail with InvalidPdf");
    assert!(matches!(cor_err, SheafError::InvalidPdf { .. }));

    // Cancelled
    let cancel_out = temp.path().join("cancel_out.pdf");
    let cancel_err = compress_with_options(
        &photo_pdf,
        &CompressOptions {
            load: LoadOptions::none(),
            preset: CompressPreset::Medium,
            grayscale: false,
            out: Some(cancel_out),
            force: false,
            dry_run: false,
        },
        &CancelledProgress,
    )
    .expect_err("should fail with Cancelled");
    assert!(matches!(cancel_err, SheafError::Cancelled));
}
