//! PDF merge operations.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lopdf::{Bookmark, Dictionary, Document, Object, ObjectId};
use serde::Serialize;

use crate::error::ensure_not_overwriting_inputs;
use crate::fs::{check_output_path, write_atomic};
use crate::{Progress, SheafError};

/// Options for merging PDF files.
#[derive(Debug, Clone)]
pub struct MergeOptions {
    /// Overwrite existing output file if `true`.
    pub force: bool,
    /// Validate inputs and compute report without writing the output file.
    pub dry_run: bool,
}

impl Default for MergeOptions {
    fn default() -> Self {
        Self {
            force: true,
            dry_run: false,
        }
    }
}

/// Report returned after merging PDF files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MergeReport {
    /// Input PDF file paths in merge order.
    pub inputs: Vec<PathBuf>,
    /// Output PDF file path.
    pub output: PathBuf,
    /// Total number of pages in the merged PDF.
    pub pages_total: u32,
}

/// Merges multiple PDF files into a single output PDF file (overwriting `out` if it exists,
/// unless `out` is one of the input files).
///
/// For fine-grained control over `force` and `dry_run` as well as receiving a [`MergeReport`],
/// see [`merge_with_options`].
///
/// # Errors
///
/// Returns:
/// - [`SheafError::EmptySelection`] if `inputs` is empty.
/// - [`SheafError::Encrypted`] if any input document is password-protected.
/// - [`SheafError::InvalidPdf`] if an input is not a valid PDF.
/// - [`SheafError::InvalidSelection`] if `out` matches an input path.
/// - [`SheafError::Cancelled`] if `p.is_cancelled()` becomes `true`.
/// - [`SheafError::Io`] on filesystem read/write errors.
///
/// # Examples
///
/// ```no_run
/// use std::path::{Path, PathBuf};
/// use sheaf_core::{merge, NoProgress};
///
/// let inputs = vec![PathBuf::from("part1.pdf"), PathBuf::from("part2.pdf")];
/// merge(&inputs, Path::new("merged.pdf"), &NoProgress).unwrap();
/// ```
pub fn merge(inputs: &[PathBuf], out: &Path, p: &dyn Progress) -> Result<(), SheafError> {
    let opts = MergeOptions {
        force: true,
        dry_run: false,
    };
    let _report = merge_with_options(inputs, out, &opts, p)?;
    Ok(())
}

/// Merges multiple PDF files into `out` according to [`MergeOptions`], returning a [`MergeReport`].
///
/// Uses atomic file writing when `opts.dry_run` is `false`.
///
/// # Errors
///
/// Returns [`SheafError::OutputExists`] if `out` exists and `!opts.force`.
///
/// # Examples
///
/// ```no_run
/// use std::path::{Path, PathBuf};
/// use sheaf_core::ops::merge::{merge_with_options, MergeOptions};
/// use sheaf_core::NoProgress;
///
/// let inputs = vec![PathBuf::from("a.pdf"), PathBuf::from("b.pdf")];
/// let opts = MergeOptions { force: false, dry_run: true };
/// let report = merge_with_options(&inputs, Path::new("out.pdf"), &opts, &NoProgress).unwrap();
/// println!("Total pages: {}", report.pages_total);
/// ```
pub fn merge_with_options(
    inputs: &[PathBuf],
    out: &Path,
    opts: &MergeOptions,
    p: &dyn Progress,
) -> Result<MergeReport, SheafError> {
    if inputs.is_empty() {
        return Err(SheafError::EmptySelection);
    }

    ensure_not_overwriting_inputs(inputs, out)?;
    check_output_path(out, opts.force)?;

    let total_steps = (inputs.len() as u64) + 1;
    p.report(0, total_steps);

    let mut max_id = 1u32;
    let mut documents_pages: Vec<(ObjectId, Object)> = Vec::new();
    let mut documents_objects: BTreeMap<ObjectId, Object> = BTreeMap::new();
    let mut merged_doc = Document::with_version("1.5");

    for (idx, path) in inputs.iter().enumerate() {
        if p.is_cancelled() {
            return Err(SheafError::Cancelled);
        }

        if !path.exists() {
            return Err(SheafError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Datei nicht gefunden: {}", path.display()),
            )));
        }

        let mut doc =
            Document::load(path).map_err(|e| SheafError::from_lopdf_with_path(e, path))?;

        if doc.is_encrypted() {
            return Err(SheafError::Encrypted);
        }

        doc.renumber_objects_with(max_id);
        max_id = doc.max_id.saturating_add(1);

        import_document_bookmarks(&doc, &mut merged_doc);

        let pages = doc.get_pages();
        if pages.is_empty() {
            return Err(SheafError::InvalidPdf(format!(
                "{}: Dokument enthält keine Seiten",
                path.display()
            )));
        }

        for page_id in pages.into_values() {
            let page_obj = doc
                .get_object(page_id)
                .map_err(|e| SheafError::from_lopdf_with_path(e, path))?
                .to_owned();
            documents_pages.push((page_id, page_obj));
        }

        documents_objects.extend(doc.objects);
        p.report((idx as u64) + 1, total_steps);
    }

    if p.is_cancelled() {
        return Err(SheafError::Cancelled);
    }

    let pages_total = u32::try_from(documents_pages.len()).unwrap_or(u32::MAX);

    if opts.dry_run {
        p.report(total_steps, total_steps);
        return Ok(MergeReport {
            inputs: inputs.to_vec(),
            output: out.to_path_buf(),
            pages_total,
        });
    }

    let mut catalog_object: Option<(ObjectId, Dictionary)> = None;
    let mut pages_object: Option<(ObjectId, Dictionary)> = None;

    for (object_id, object) in documents_objects {
        match object.type_name().unwrap_or(b"") {
            b"Catalog" => {
                if let Ok(dict) = object.as_dict()
                    && catalog_object.is_none()
                {
                    catalog_object = Some((object_id, dict.clone()));
                }
            }
            b"Pages" => {
                if let Ok(dict) = object.as_dict() {
                    if let Some((existing_id, mut existing_dict)) = pages_object.take() {
                        for (k, v) in dict.iter() {
                            if !existing_dict.has(k) {
                                existing_dict.set(k.clone(), v.clone());
                            }
                        }
                        pages_object = Some((existing_id, existing_dict));
                    } else {
                        pages_object = Some((object_id, dict.clone()));
                    }
                }
            }
            b"Page" | b"Outlines" | b"Outline" => {}
            _ => {
                merged_doc.objects.insert(object_id, object);
            }
        }
    }

    let (pages_id, mut pages_dict) = pages_object.ok_or_else(|| {
        SheafError::InvalidPdf("Kein gültiges Pages-Objekt in den Eingabedateien".to_string())
    })?;

    let (catalog_id, mut catalog_dict) = catalog_object.ok_or_else(|| {
        SheafError::InvalidPdf("Kein gültiges Catalog-Objekt in den Eingabedateien".to_string())
    })?;

    let mut kids = Vec::with_capacity(documents_pages.len());
    for (page_id, page_obj) in &documents_pages {
        if let Ok(dict) = page_obj.as_dict() {
            let mut page_dict = dict.clone();
            page_dict.set("Parent", pages_id);
            merged_doc
                .objects
                .insert(*page_id, Object::Dictionary(page_dict));
            kids.push(Object::Reference(*page_id));
        }
    }

    pages_dict.set("Count", Object::Integer(kids.len() as i64));
    pages_dict.set("Kids", Object::Array(kids));
    pages_dict.remove(b"Parent");
    merged_doc
        .objects
        .insert(pages_id, Object::Dictionary(pages_dict));

    catalog_dict.set("Pages", pages_id);
    catalog_dict.remove(b"Outlines");
    catalog_dict.remove(b"Dests");
    catalog_dict.remove(b"Names");

    let has_bookmarks = !merged_doc.bookmarks.is_empty();
    if has_bookmarks {
        catalog_dict.set("PageMode", Object::Name(b"UseOutlines".to_vec()));
    }

    merged_doc
        .objects
        .insert(catalog_id, Object::Dictionary(catalog_dict));
    merged_doc.trailer.set("Root", catalog_id);

    merged_doc.max_id = merged_doc
        .objects
        .keys()
        .map(|(id, _)| *id)
        .max()
        .unwrap_or(1);

    merged_doc.renumber_objects();
    merged_doc.adjust_zero_pages();

    if let Some(outline_id) = merged_doc.build_outline()
        && let Ok(Object::Dictionary(dict)) = merged_doc.get_object_mut(catalog_id)
    {
        dict.set("Outlines", Object::Reference(outline_id));
    }

    write_atomic(out, opts.force, |file| {
        merged_doc.save_to(file)?;
        Ok(())
    })?;

    p.report(total_steps, total_steps);
    Ok(MergeReport {
        inputs: inputs.to_vec(),
        output: out.to_path_buf(),
        pages_total,
    })
}

/// Extracts bookmarks/outlines from `source` and appends them into `target`,
/// preserving nesting levels and target page object IDs.
fn import_document_bookmarks(source: &Document, target: &mut Document) {
    if !source.bookmarks.is_empty() {
        copy_in_memory_bookmarks(source, &source.bookmarks, None, target);
        return;
    }

    let Ok(toc) = source.get_toc() else {
        return;
    };
    if toc.toc.is_empty() {
        return;
    }

    let pages = source.get_pages();
    let mut level_parents: BTreeMap<usize, u32> = BTreeMap::new();

    for entry in toc.toc {
        let page_num = u32::try_from(entry.page).unwrap_or(1);
        let Some(&page_obj_id) = pages.get(&page_num) else {
            continue;
        };

        let level = entry.level.max(1);
        let parent_id = if level > 1 {
            level_parents.range(1..level).next_back().map(|(_, &id)| id)
        } else {
            None
        };

        let bm = Bookmark::new(entry.title, [0.0, 0.0, 0.0], 0, page_obj_id);
        let new_id = target.add_bookmark(bm, parent_id);
        level_parents.insert(level, new_id);
        level_parents.retain(|&lvl, _| lvl <= level);
    }
}

fn copy_in_memory_bookmarks(
    source: &Document,
    bookmark_ids: &[u32],
    parent: Option<u32>,
    target: &mut Document,
) {
    for &id in bookmark_ids {
        if let Some(bm) = source.bookmark_table.get(&id) {
            let new_bm = Bookmark::new(bm.title.clone(), bm.color, bm.format, bm.page);
            let new_id = target.add_bookmark(new_bm, parent);
            if !bm.children.is_empty() {
                copy_in_memory_bookmarks(source, &bm.children, Some(new_id), target);
            }
        }
    }
}
