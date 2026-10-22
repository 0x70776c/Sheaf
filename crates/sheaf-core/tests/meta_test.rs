use proptest::prelude::*;
use std::collections::BTreeMap;
use std::path::PathBuf;

use sheaf_core::{
    LoadOptions, MetaGetOptions, MetaSetOptions, MetaStripOptions, NoProgress, XmpMode,
    decode_pdf_string, encode_pdf_string, load_pdf, meta_get, meta_set, meta_strip, parse_pdf_date,
};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests")
        .join("fixtures")
}

// ----------------------------------------------------------------------------
// 1. Property tests (proptest)
// ----------------------------------------------------------------------------

proptest! {
    /// Decode(Encode(s)) must round-trip back to s for any arbitrary Unicode string.
    #[test]
    fn prop_unicode_encode_decode_roundtrip(s in "\\PC*") {
        let encoded_bytes = encode_pdf_string(&s);
        let decoded = decode_pdf_string(&encoded_bytes);
        prop_assert_eq!(decoded, s);
    }

    /// Date parser must never panic on arbitrary string inputs.
    #[test]
    fn prop_date_parser_never_panics(s in ".*") {
        let _ = parse_pdf_date(&s);
    }
}

// ----------------------------------------------------------------------------
// 2. Unicode and emoji roundtrip in meta_set -> meta_get
// ----------------------------------------------------------------------------

#[test]
fn test_meta_set_get_roundtrip_unicode_cjk_emoji() {
    let src = fixtures_dir().join("fixture_a_2p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");
    let out = temp.path().join("unicode_meta.pdf");

    let title_val = "Testbericht für Müller & Söhne";
    let author_val = "山田太郎 (Yamada Taro) 日本語";
    let subject_val = "Sheaf Release Party 🚀🎉";
    let keywords_val = "Rust, PDF, 🦀, 2026";

    let mut custom = BTreeMap::new();
    custom.insert("Abteilung".to_string(), "Forschung & Entwicklung".to_string());
    custom.insert("Emoji-Key".to_string(), "🌟 Star 🌟".to_string());

    let set_opts = MetaSetOptions {
        load: LoadOptions::none(),
        title: Some(title_val.to_string()),
        author: Some(author_val.to_string()),
        subject: Some(subject_val.to_string()),
        keywords: Some(keywords_val.to_string()),
        creator: Some("Sheaf Desktop 0.1".to_string()),
        producer: Some("Sheaf Core Engine".to_string()),
        creation_date: None,
        mod_date: "now".to_string(),
        custom,
        unset: Vec::new(),
        xmp: XmpMode::Sync,
        out: Some(out.clone()),
        force: true,
        dry_run: false,
    };

    let set_rep = meta_set(&src, &set_opts, &NoProgress).expect("meta_set failed");
    assert!(out.exists());
    assert!(!set_rep.diff.is_empty());

    // Verify through meta_get
    let get_opts = MetaGetOptions {
        load: LoadOptions::none(),
        include_xmp_packet: true,
    };
    let get_rep = meta_get(&[out.clone()], &get_opts, &NoProgress).expect("meta_get failed");
    assert_eq!(get_rep.files.len(), 1);
    let f = &get_rep.files[0];

    assert_eq!(f.info.title.as_deref(), Some(title_val));
    assert_eq!(f.info.author.as_deref(), Some(author_val));
    assert_eq!(f.info.subject.as_deref(), Some(subject_val));
    assert_eq!(f.info.keywords.as_deref(), Some(keywords_val));
    assert_eq!(f.info.creator.as_deref(), Some("Sheaf Desktop 0.1"));
    assert_eq!(f.info.producer.as_deref(), Some("Sheaf Core Engine"));

    assert_eq!(
        f.custom.get("Abteilung").map(|s| s.as_str()),
        Some("Forschung & Entwicklung")
    );
    assert_eq!(
        f.custom.get("Emoji-Key").map(|s| s.as_str()),
        Some("🌟 Star 🌟")
    );
}

// ----------------------------------------------------------------------------
// 3. Determinism with SOURCE_DATE_EPOCH
// ----------------------------------------------------------------------------

#[test]
fn test_meta_set_deterministic_with_source_date_epoch() {
    let src = fixtures_dir().join("fixture_a_2p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");
    let out1 = temp.path().join("epoch_out1.pdf");
    let out2 = temp.path().join("epoch_out2.pdf");

    unsafe {
        std::env::set_var("SOURCE_DATE_EPOCH", "1700000000");
    }

    let opts1 = MetaSetOptions {
        load: LoadOptions::none(),
        title: Some("Deterministic Title".to_string()),
        author: Some("Deterministic Author".to_string()),
        subject: None,
        keywords: None,
        creator: None,
        producer: None,
        creation_date: None,
        mod_date: "now".to_string(),
        custom: BTreeMap::new(),
        unset: Vec::new(),
        xmp: XmpMode::Sync,
        out: Some(out1.clone()),
        force: true,
        dry_run: false,
    };

    let opts2 = MetaSetOptions {
        out: Some(out2.clone()),
        ..opts1.clone()
    };

    meta_set(&src, &opts1, &NoProgress).expect("run 1");
    meta_set(&src, &opts2, &NoProgress).expect("run 2");

    unsafe {
        std::env::remove_var("SOURCE_DATE_EPOCH");
    }

    let bytes1 = std::fs::read(&out1).expect("read out1");
    let bytes2 = std::fs::read(&out2).expect("read out2");

    assert_eq!(
        bytes1, bytes2,
        "Two runs with same SOURCE_DATE_EPOCH must produce byte-identical files"
    );
}

// ----------------------------------------------------------------------------
// 4. Content streams remain byte-identical before and after set/strip
// ----------------------------------------------------------------------------

#[test]
fn test_meta_operations_preserve_content_stream_bytes() {
    let src = fixtures_dir().join("fixture_b_3p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");
    let set_out = temp.path().join("content_set.pdf");
    let strip_out = temp.path().join("content_strip.pdf");

    let orig = load_pdf(&src, &LoadOptions::none()).expect("load orig");
    let orig_pages = orig.doc.get_pages();

    // 1. Run meta_set
    let set_opts = MetaSetOptions {
        load: LoadOptions::none(),
        title: Some("New Title".to_string()),
        author: Some("New Author".to_string()),
        subject: None,
        keywords: None,
        creator: None,
        producer: None,
        creation_date: None,
        mod_date: "now".to_string(),
        custom: BTreeMap::new(),
        unset: Vec::new(),
        xmp: XmpMode::Sync,
        out: Some(set_out.clone()),
        force: true,
        dry_run: false,
    };
    meta_set(&src, &set_opts, &NoProgress).expect("meta set");

    let after_set = load_pdf(&set_out, &LoadOptions::none()).expect("load after set");
    let set_pages = after_set.doc.get_pages();
    assert_eq!(orig_pages.len(), set_pages.len());

    for p in 1..=orig_pages.len() as u32 {
        let p_id_orig = *orig_pages.get(&p).unwrap();
        let p_id_set = *set_pages.get(&p).unwrap();
        let c_orig = orig.doc.get_page_content(p_id_orig);
        let c_set = after_set.doc.get_page_content(p_id_set);
        assert_eq!(c_orig, c_set, "Page {p} content must not change after meta_set");
    }

    // 2. Run meta_strip
    let strip_opts = MetaStripOptions {
        load: LoadOptions::none(),
        deep: true,
        keep: Vec::new(),
        out: Some(strip_out.clone()),
        force: true,
        dry_run: false,
    };
    meta_strip(&src, &strip_opts, &NoProgress).expect("meta strip");

    let after_strip = load_pdf(&strip_out, &LoadOptions::none()).expect("load after strip");
    let strip_pages = after_strip.doc.get_pages();
    assert_eq!(orig_pages.len(), strip_pages.len());

    for p in 1..=orig_pages.len() as u32 {
        let p_id_orig = *orig_pages.get(&p).unwrap();
        let p_id_strip = *strip_pages.get(&p).unwrap();
        let c_orig = orig.doc.get_page_content(p_id_orig);
        let c_strip = after_strip.doc.get_page_content(p_id_strip);
        assert_eq!(c_orig, c_strip, "Page {p} content must not change after meta_strip");
    }
}

// ----------------------------------------------------------------------------
// 5. Deep scan after strip: no metadata or PieceInfo in any object
// ----------------------------------------------------------------------------

#[test]
fn test_meta_strip_removes_all_metadata_and_piece_info() {
    let src = fixtures_dir().join("fixture_12p.pdf");
    let temp = tempfile::tempdir().expect("tempdir");
    let out = temp.path().join("clean_deep.pdf");

    let strip_opts = MetaStripOptions {
        load: LoadOptions::none(),
        deep: true,
        keep: Vec::new(),
        out: Some(out.clone()),
        force: true,
        dry_run: false,
    };

    let rep = meta_strip(&src, &strip_opts, &NoProgress).expect("strip");
    assert!(rep.removed.trailer_id_regenerated);

    let loaded = load_pdf(&out, &LoadOptions::none()).expect("load stripped");

    // Trailer Info must be removed
    assert!(loaded.doc.trailer.get(b"Info").is_err());

    // Catalog must not have /Metadata or /PieceInfo
    let root_id = loaded.doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let root_dict = loaded.doc.get_dictionary(root_id).unwrap();
    assert!(!root_dict.has(b"Metadata"));
    assert!(!root_dict.has(b"PieceInfo"));

    // Pages must not have /PieceInfo or /Metadata
    for (_p, &p_id) in loaded.doc.get_pages().iter() {
        let p_dict = loaded.doc.get_dictionary(p_id).unwrap();
        assert!(!p_dict.has(b"Metadata"));
        assert!(!p_dict.has(b"PieceInfo"));
    }
}
