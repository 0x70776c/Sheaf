use std::path::PathBuf;

use image::GenericImageView;
use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, Stream, StringFormat, dictionary};
use sheaf_core::{
    NoProgress, NumberOptions, NumberPosition, OverlayPosition, PageOverlaySpec, RenderOptions,
    RenderScale, SheafError, WatermarkOptions, WatermarkPosition, apply_overlay_to_page,
    format_page_number, get_effective_page_box, is_pdfium_available, number, render_with_options,
    watermark,
};
use tempfile::TempDir;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn test_overlay_audit_01_shared_engine() {
    let dir = TempDir::new().unwrap();
    let src = fixture_path("fixture_b_3p.pdf");
    let out = dir.path().join("wm_shared.pdf");

    let opts = WatermarkOptions {
        text: "AUDIT-01".to_string(),
        position: WatermarkPosition::BottomRight,
        out: Some(out.clone()),
        ..WatermarkOptions::default()
    };
    let report = watermark(&src, &opts, &NoProgress).unwrap();
    assert_eq!(report.pages_watermarked, vec![1, 2, 3]);

    let doc = Document::load(&out).unwrap();
    let page1_id = *doc.get_pages().get(&1).unwrap();
    let page_dict = doc.get_dictionary(page1_id).unwrap();
    let res = page_dict.get(b"Resources").unwrap().as_dict().unwrap();
    let fonts = res.get(b"Font").unwrap().as_dict().unwrap();
    assert!(
        fonts.has(b"SheafF0"),
        "watermark must allocate SheafF0 via ops::overlay"
    );
}

#[test]
fn test_overlay_audit_02_original_content_encapsulated_in_q_q() {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join("unbalanced_ctm.pdf");
    let out = dir.path().join("unbalanced_ctm_wm.pdf");

    // Build a PDF whose original content stream modifies the CTM (`0.05 0 0 0.05 400 400 cm`)
    // without a matching `Q`.
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let content_bytes = Content {
        operations: vec![Operation::new(
            "cm",
            vec![
                0.05.into(),
                0.into(),
                0.into(),
                0.05.into(),
                400.into(),
                400.into(),
            ],
        )],
    }
    .encode()
    .unwrap();
    let content_id = doc.add_object(Stream::new(dictionary! {}, content_bytes));
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => content_id,
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    doc.save(&src).unwrap();

    let opts = WatermarkOptions {
        text: "ENCAPSULATED".to_string(),
        out: Some(out.clone()),
        ..WatermarkOptions::default()
    };
    watermark(&src, &opts, &NoProgress).unwrap();

    let res_doc = Document::load(&out).unwrap();
    let res_page_id = *res_doc.get_pages().get(&1).unwrap();
    let res_page = res_doc.get_dictionary(res_page_id).unwrap();
    let contents_arr = res_page.get(b"Contents").unwrap().as_array().unwrap();
    assert_eq!(
        contents_arr.len(),
        3,
        "Contents should be [q_prefix, original, Q_and_overlay]"
    );

    let first_stream = res_doc
        .get_object(contents_arr[0].as_reference().unwrap())
        .unwrap()
        .as_stream()
        .unwrap();
    let first_ops = Content::decode(&first_stream.decompressed_content().unwrap())
        .unwrap()
        .operations;
    assert_eq!(first_ops[0].operator, "q");

    let last_stream = res_doc
        .get_object(contents_arr[2].as_reference().unwrap())
        .unwrap()
        .as_stream()
        .unwrap();
    let last_ops = Content::decode(&last_stream.decompressed_content().unwrap())
        .unwrap()
        .operations;
    assert_eq!(
        last_ops[0].operator, "Q",
        "Suffix stream must start with Q to close original content's q"
    );
}

#[test]
fn test_overlay_audit_03_inherited_and_indirect_resources_copied_parent_unmodified() {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join("inherited_res.pdf");
    let out = dir.path().join("inherited_res_wm.pdf");

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let orig_font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier",
    });
    let shared_font_dict_id = doc.add_object(dictionary! {
        "FOrig" => orig_font_id,
    });
    let shared_res_id = doc.add_object(dictionary! {
        "Font" => shared_font_dict_id,
    });

    let page1_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });
    let page2_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page1_id.into(), page2_id.into()],
            "Count" => 2,
            "Resources" => shared_res_id,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    doc.save(&src).unwrap();

    // Watermark only page 1
    let opts = WatermarkOptions {
        text: "PAGE1-ONLY".to_string(),
        pages: Some("1".to_string()),
        out: Some(out.clone()),
        ..WatermarkOptions::default()
    };
    watermark(&src, &opts, &NoProgress).unwrap();

    let res_doc = Document::load(&out).unwrap();
    let pages_map = res_doc.get_pages();
    let p1 = res_doc.get_dictionary(*pages_map.get(&1).unwrap()).unwrap();
    let p1_res = p1.get(b"Resources").unwrap().as_dict().unwrap();
    let p1_fonts = p1_res.get(b"Font").unwrap().as_dict().unwrap();
    assert!(
        p1_fonts.has(b"FOrig"),
        "Inherited FOrig must be preserved on page 1"
    );
    assert!(p1_fonts.has(b"SheafF0"), "SheafF0 must be added to page 1");

    // Verify Parent /Resources and shared_font_dict_id are untouched
    let parent_id = p1.get(b"Parent").unwrap().as_reference().unwrap();
    let parent_dict = res_doc.get_dictionary(parent_id).unwrap();
    let parent_res_ref = parent_dict
        .get(b"Resources")
        .unwrap()
        .as_reference()
        .unwrap();
    let parent_res = res_doc.get_dictionary(parent_res_ref).unwrap();
    let parent_font_ref = parent_res.get(b"Font").unwrap().as_reference().unwrap();
    let parent_fonts = res_doc.get_dictionary(parent_font_ref).unwrap();
    assert!(parent_fonts.has(b"FOrig"));
    assert!(
        !parent_fonts.has(b"SheafF0"),
        "Parent /Resources must NOT be mutated by page 1 overlay"
    );
}

#[test]
fn test_overlay_audit_04a_contents_single_reference() {
    let dir = TempDir::new().unwrap();
    let src = fixture_path("fixture_c_1p.pdf");
    let out = dir.path().join("single_ref_wm.pdf");

    let doc_in = Document::load(&src).unwrap();
    let p1_in = doc_in
        .get_dictionary(*doc_in.get_pages().get(&1).unwrap())
        .unwrap();
    assert!(matches!(
        p1_in.get(b"Contents").unwrap(),
        Object::Reference(_)
    ));

    let opts = WatermarkOptions {
        text: "REF-TEST".to_string(),
        out: Some(out.clone()),
        ..WatermarkOptions::default()
    };
    watermark(&src, &opts, &NoProgress).unwrap();

    let doc_out = Document::load(&out).unwrap();
    let extracted = doc_out.extract_text(&[1]).unwrap();
    assert!(extracted.contains("REF-TEST"));
}

#[test]
fn test_overlay_audit_04b_contents_array() {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join("array_contents.pdf");
    let out = dir.path().join("array_contents_wm.pdf");

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let s1 = doc.add_object(Stream::new(
        dictionary! {},
        Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 12.into()]),
                Operation::new("Td", vec![72.into(), 700.into()]),
                Operation::new(
                    "Tj",
                    vec![Object::String(b"PartOne ".to_vec(), StringFormat::Literal)],
                ),
                Operation::new("ET", vec![]),
            ],
        }
        .encode()
        .unwrap(),
    ));
    let s2 = doc.add_object(Stream::new(
        dictionary! {},
        Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 12.into()]),
                Operation::new("Td", vec![140.into(), 700.into()]),
                Operation::new(
                    "Tj",
                    vec![Object::String(b"PartTwo".to_vec(), StringFormat::Literal)],
                ),
                Operation::new("ET", vec![]),
            ],
        }
        .encode()
        .unwrap(),
    ));
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => dictionary! { "Font" => dictionary! { "F1" => font_id } },
        "Contents" => vec![Object::Reference(s1), Object::Reference(s2)],
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    doc.save(&src).unwrap();

    let opts = WatermarkOptions {
        text: "ARRAY-WM".to_string(),
        out: Some(out.clone()),
        ..WatermarkOptions::default()
    };
    watermark(&src, &opts, &NoProgress).unwrap();

    let doc_out = Document::load(&out).unwrap();
    let text = doc_out.extract_text(&[1]).unwrap();
    assert!(text.contains("PartOne"));
    assert!(text.contains("PartTwo"));
    assert!(text.contains("ARRAY-WM"));
}

#[test]
fn test_overlay_audit_04c_contents_empty() {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join("empty_contents.pdf");
    let out = dir.path().join("empty_contents_wm.pdf");

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    doc.save(&src).unwrap();

    let opts = WatermarkOptions {
        text: "EMPTY-PAGE-WM".to_string(),
        out: Some(out.clone()),
        ..WatermarkOptions::default()
    };
    watermark(&src, &opts, &NoProgress).unwrap();

    let doc_out = Document::load(&out).unwrap();
    let text = doc_out.extract_text(&[1]).unwrap();
    assert!(text.contains("EMPTY-PAGE-WM"));
}

#[test]
fn test_overlay_audit_05_rotate_90_180_270_visual_pixel_check() {
    if !is_pdfium_available() {
        return;
    }
    let dir = TempDir::new().unwrap();

    for &rot in &[90i32, 180, 270] {
        let src = dir.path().join(format!("rot_{rot}.pdf"));
        let out = dir.path().join(format!("rot_{rot}_wm.pdf"));
        let png = dir.path().join(format!("rot_{rot}_wm.png"));

        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Rotate" => rot,
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page_id.into()],
                "Count" => 1,
            }),
        );
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);
        doc.save(&src).unwrap();

        let opts = WatermarkOptions {
            text: "TOPLEFT".to_string(),
            position: WatermarkPosition::TopLeft,
            font_size: 48.0,
            opacity: 1.0,
            color: "#FF0000".to_string(),
            out: Some(out.clone()),
            ..WatermarkOptions::default()
        };
        watermark(&src, &opts, &NoProgress).unwrap();

        let render_opts = RenderOptions {
            pages: Some("1".to_string()),
            scale: RenderScale::Dpi(72),
            out: png.clone(),
            force: true,
            dry_run: false,
            use_cache: false,
            cache_dir: None,
        };
        render_with_options(&out, &render_opts, &NoProgress).unwrap();

        let img = image::open(&png).unwrap();
        let (w, h) = img.dimensions();
        // Count red pixels in top-left quadrant vs bottom-right quadrant
        let mut top_left_red = 0u32;
        let mut bottom_right_red = 0u32;
        for y in 0..h {
            for x in 0..w {
                let p = img.get_pixel(x, y);
                if p[0] > 200 && p[1] < 80 && p[2] < 80 {
                    if x < w / 2 && y < h / 3 {
                        top_left_red += 1;
                    } else if x >= w / 2 && y >= (2 * h) / 3 {
                        bottom_right_red += 1;
                    }
                }
            }
        }
        assert!(
            top_left_red > 50 && bottom_right_red == 0,
            "For Rotate={rot}, TopLeft watermark must render in visual top-left (got top_left_red={top_left_red}, bottom_right_red={bottom_right_red})"
        );
    }
}

#[test]
fn test_overlay_audit_06_mediabox_offset_and_cropbox() {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join("offset_crop.pdf");

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![100.into(), 200.into(), 712.into(), 992.into()],
        "CropBox" => vec![150.into(), 250.into(), 550.into(), 650.into()],
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    doc.save(&src).unwrap();

    let loaded = Document::load(&src).unwrap();
    let pid = *loaded.get_pages().get(&1).unwrap();
    let pbox = get_effective_page_box(&loaded, pid);
    assert_eq!(pbox.x0, 150.0);
    assert_eq!(pbox.y0, 250.0);
    assert_eq!(pbox.width, 400.0);
    assert_eq!(pbox.height, 400.0);
    assert_eq!(
        pbox.visual_to_user_ctm(),
        [1.0, 0.0, 0.0, 1.0, 150.0, 250.0]
    );
}

#[test]
fn test_overlay_audit_07_resource_names_collision_free() {
    let dir = TempDir::new().unwrap();
    let src = fixture_path("fixture_c_1p.pdf");
    let mid = dir.path().join("wm1.pdf");
    let out = dir.path().join("wm2.pdf");

    watermark(
        &src,
        &WatermarkOptions {
            text: "FIRST".to_string(),
            opacity: 0.25,
            out: Some(mid.clone()),
            ..WatermarkOptions::default()
        },
        &NoProgress,
    )
    .unwrap();

    watermark(
        &mid,
        &WatermarkOptions {
            text: "SECOND".to_string(),
            opacity: 0.80,
            out: Some(out.clone()),
            ..WatermarkOptions::default()
        },
        &NoProgress,
    )
    .unwrap();

    let doc = Document::load(&out).unwrap();
    let pid = *doc.get_pages().get(&1).unwrap();
    let res = doc
        .get_dictionary(pid)
        .unwrap()
        .get(b"Resources")
        .unwrap()
        .as_dict()
        .unwrap();
    let fonts = res.get(b"Font").unwrap().as_dict().unwrap();
    let gs = res.get(b"ExtGState").unwrap().as_dict().unwrap();
    assert!(fonts.has(b"SheafF0"));
    assert!(fonts.has(b"SheafF1"));
    assert!(gs.has(b"SheafGS0"));
    assert!(gs.has(b"SheafGS1"));
}

#[test]
fn test_overlay_audit_08_non_winansi_returns_unsupported_with_hint() {
    let dir = TempDir::new().unwrap();
    let src = fixture_path("fixture_c_1p.pdf");
    let out = dir.path().join("emoji.pdf");

    let err = watermark(
        &src,
        &WatermarkOptions {
            text: "SECRET 🔒".to_string(),
            out: Some(out),
            ..WatermarkOptions::default()
        },
        &NoProgress,
    )
    .unwrap_err();

    match err {
        SheafError::Unsupported(msg) => {
            assert!(msg.contains("WinAnsiEncoding"));
            assert!(msg.contains("Hint:"));
        }
        other => panic!("Expected SheafError::Unsupported, got: {other:?}"),
    }
}

#[test]
fn test_overlay_audit_09_opacity_via_extgstate_ca() {
    let dir = TempDir::new().unwrap();
    let src = fixture_path("fixture_c_1p.pdf");
    let out = dir.path().join("opacity.pdf");

    watermark(
        &src,
        &WatermarkOptions {
            text: "OPACITY-CHECK".to_string(),
            opacity: 0.35,
            out: Some(out.clone()),
            ..WatermarkOptions::default()
        },
        &NoProgress,
    )
    .unwrap();

    let doc = Document::load(&out).unwrap();
    let pid = *doc.get_pages().get(&1).unwrap();
    let res = doc
        .get_dictionary(pid)
        .unwrap()
        .get(b"Resources")
        .unwrap()
        .as_dict()
        .unwrap();
    let gs_dict = res.get(b"ExtGState").unwrap().as_dict().unwrap();
    let gs0_ref = gs_dict.get(b"SheafGS0").unwrap().as_reference().unwrap();
    let gs0 = doc.get_dictionary(gs0_ref).unwrap();
    let ca_lower = gs0.get(b"ca").unwrap().as_f32().unwrap();
    let ca_upper = gs0.get(b"CA").unwrap().as_f32().unwrap();
    assert!((ca_lower - 0.35).abs() < 1e-4);
    assert!((ca_upper - 0.35).abs() < 1e-4);
}

#[test]
fn test_overlay_audit_10_text_remains_selectable() {
    let dir = TempDir::new().unwrap();
    let src = fixture_path("fixture_c_1p.pdf");
    let out = dir.path().join("selectable.pdf");

    let orig_doc = Document::load(&src).unwrap();
    let orig_text = orig_doc.extract_text(&[1]).unwrap();

    let mut doc = orig_doc;
    let pid = *doc.get_pages().get(&1).unwrap();
    let (font_id, gs_id) =
        sheaf_core::ops::overlay::create_overlay_shared_objects(&mut doc, "Helvetica-Bold", 0.5);
    apply_overlay_to_page(
        &mut doc,
        pid,
        font_id,
        gs_id,
        true,
        &PageOverlaySpec {
            text: "SELECTABLE-OVERLAY-TEXT",
            position: OverlayPosition::BottomCenter,
            font_size: 14.0,
            rotation_deg: 0.0,
            rgb: (0.2, 0.2, 0.2),
            margin: 36.0,
        },
    )
    .unwrap();
    doc.save(&out).unwrap();

    let reloaded = Document::load(&out).unwrap();
    let new_text = reloaded.extract_text(&[1]).unwrap();
    assert!(new_text.contains(orig_text.trim()));
    assert!(new_text.contains("SELECTABLE-OVERLAY-TEXT"));
}

#[test]
fn test_number_placeholders_and_subset_pages() {
    let dir = TempDir::new().unwrap();
    let src = fixture_path("fixture_b_3p.pdf");
    let out = dir.path().join("numbered.pdf");

    let opts = NumberOptions {
        format: "N={n} T={total} P={page} ALL={pages}".to_string(),
        position: NumberPosition::BottomRight,
        start: 10,
        pages: Some("2-3".to_string()),
        out: Some(out.clone()),
        ..NumberOptions::default()
    };

    let report = number(&src, &opts, &NoProgress).unwrap();
    assert_eq!(report.pages_numbered, vec![2, 3]);
    assert_eq!(report.total_pages, 3);

    let doc = Document::load(&out).unwrap();
    let p1_text = doc.extract_text(&[1]).unwrap();
    let p2_text = doc.extract_text(&[2]).unwrap();
    let p3_text = doc.extract_text(&[3]).unwrap();

    assert!(!p1_text.contains("N=10"));
    assert!(p2_text.contains("N=10 T=2 P=2 ALL=3"));
    assert!(p3_text.contains("N=11 T=2 P=3 ALL=3"));
}

#[test]
fn test_number_default_output_and_invalid_placeholder() {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join("doc.pdf");
    std::fs::copy(fixture_path("fixture_c_1p.pdf"), &src).unwrap();

    let report = number(&src, &NumberOptions::default(), &NoProgress).unwrap();
    assert_eq!(report.output, dir.path().join("doc_numbered.pdf"));
    assert!(report.output.exists());

    assert!(format_page_number("{bad}", 1, 1, 1, 1).is_err());
    assert!(format_page_number("unclosed {n", 1, 1, 1, 1).is_err());
}
