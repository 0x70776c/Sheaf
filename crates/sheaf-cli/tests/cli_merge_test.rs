use std::path::{Path, PathBuf};
use std::process::Command;

use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, Stream, dictionary};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn create_simple_pdf(pages: &[&str]) -> Document {
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

    let mut kids = Vec::new();
    for text in pages {
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 24.into()]),
                Operation::new("Td", vec![72.into(), 720.into()]),
                Operation::new("Tj", vec![Object::string_literal(*text)]),
                Operation::new("ET", vec![]),
            ],
        };
        let stream_id = doc.add_object(Stream::new(
            dictionary! {},
            content.encode().expect("encode"),
        ));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => stream_id,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });
        kids.push(Object::Reference(page_id));
    }

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => pages.len() as i64,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    doc
}

#[test]
fn cli_merge_creates_valid_combined_pdf() {
    let root = workspace_root();
    let fixtures = root.join("tests").join("fixtures");

    let f1 = fixtures.join("fixture_a_2p.pdf");
    let f2 = fixtures.join("fixture_b_3p.pdf");
    let f3 = fixtures.join("fixture_c_1p.pdf");

    if !f1.exists() {
        std::fs::create_dir_all(&fixtures).expect("create fixtures dir");
        for (name, pages) in [
            ("fixture_a_2p.pdf", vec!["A1", "A2"]),
            ("fixture_b_3p.pdf", vec!["B1", "B2", "B3"]),
            ("fixture_c_1p.pdf", vec!["C1"]),
        ] {
            let mut doc = create_simple_pdf(&pages);
            doc.save(fixtures.join(name)).expect("save fixture");
        }
    }

    let temp = tempfile::tempdir().expect("tempdir");
    let out = temp.path().join("cli_merged.pdf");

    let bin = env!("CARGO_BIN_EXE_sheaf");
    let status = Command::new(bin)
        .arg("merge")
        .arg(&f1)
        .arg(&f2)
        .arg(&f3)
        .arg("-o")
        .arg(&out)
        .status()
        .expect("run sheaf merge");

    assert!(status.success(), "sheaf merge CLI command failed");
    assert!(out.exists(), "output PDF was not created");

    let merged = Document::load(&out).expect("load merged PDF");
    assert_eq!(merged.get_pages().len(), 6);
}
