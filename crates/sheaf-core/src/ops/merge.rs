//! PDF merge operations.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lopdf::{Bookmark, Dictionary, Document, Object, ObjectId};

use crate::error::ensure_not_overwriting_inputs;
use crate::{Progress, SheafError};

/// Merges multiple PDF files into a single output PDF file.
///
/// The page order in `out` follows the order of `inputs`. Existing bookmarks (outlines)
/// from all input documents are preserved in order, including their hierarchy.
/// Original input files are never modified.
///
/// # Errors
///
/// Returns:
/// - [`SheafError::EmptySelection`] if `inputs` is empty.
/// - [`SheafError::Encrypted`] if any input document is password-protected.
/// - [`SheafError::InvalidPdf`] if an input is not a valid PDF or if `out` matches an input path.
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
    if inputs.is_empty() {
        return Err(SheafError::EmptySelection);
    }

    ensure_not_overwriting_inputs(inputs, out)?;

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

        // Shift object IDs so they don't collide with previously loaded documents.
        doc.renumber_objects_with(max_id);
        max_id = doc.max_id.saturating_add(1);

        // Preserve bookmarks/outlines from this document after renumbering so page ObjectIds match.
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
                        // Keep initial root Pages dictionary attributes (e.g. inherited MediaBox/Resources)
                        // while only filling missing keys from subsequent documents.
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
            b"Page" | b"Outlines" | b"Outline" => {
                // Handled separately when rebuilding the Pages tree and Outlines tree.
            }
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

    if let Some(parent) = out.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }

    merged_doc.save(out)?;

    p.report(total_steps, total_steps);
    Ok(())
}

/// Extracts bookmarks/outlines from `source` and appends them into `target`,
/// preserving nesting levels and target page object IDs.
fn import_document_bookmarks(source: &Document, target: &mut Document) {
    // First check if `source` has in-memory bookmarks (e.g. via `add_bookmark`)
    if !source.bookmarks.is_empty() {
        copy_in_memory_bookmarks(source, &source.bookmarks, None, target);
        return;
    }

    // Otherwise extract from the PDF `/Outlines` dictionary via `get_toc()`
    let Ok(toc) = source.get_toc() else {
        return;
    };
    if toc.toc.is_empty() {
        return;
    }

    let pages = source.get_pages();
    // Keep track of parent bookmark IDs by 1-based level
    let mut level_parents: BTreeMap<usize, u32> = BTreeMap::new();

    for entry in toc.toc {
        let page_num = u32::try_from(entry.page).unwrap_or(1);
        let Some(&page_obj_id) = pages.get(&page_num) else {
            continue;
        };

        let level = entry.level.max(1);
        let parent_id = if level > 1 {
            // Find the closest ancestor at a higher level
            level_parents.range(1..level).next_back().map(|(_, &id)| id)
        } else {
            None
        };

        let bm = Bookmark::new(entry.title, [0.0, 0.0, 0.0], 0, page_obj_id);
        let new_id = target.add_bookmark(bm, parent_id);
        level_parents.insert(level, new_id);
        // Clear any deeper levels from previous branches
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
