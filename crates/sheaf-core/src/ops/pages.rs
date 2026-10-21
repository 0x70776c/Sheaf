//! Page-level PDF manipulation operations (`rotate`, `delete`, `reorder`, `extract`).

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use lopdf::{Document, Object, ObjectId};
use serde::Serialize;

use crate::error::ensure_not_overwriting_inputs;
use crate::fs::{check_output_path, write_atomic};
use crate::ops::split::{build_subset_document, sanitize_stem};
use crate::{PageSelection, Progress, SheafError};

/// Options for page-level operations (`rotate`, `delete`, `reorder`, `extract`).
#[derive(Debug, Clone, Default)]
pub struct PageOpOptions {
    /// Target output PDF file path. If empty, defaults to `{stem}_{suffix}.pdf` in the source directory.
    pub out: Option<PathBuf>,
    /// Overwrite existing output file if `true`.
    pub force: bool,
    /// Validate inputs and compute report without writing the output file.
    pub dry_run: bool,
}

/// Report returned by page operations (`rotate`, `delete`, `reorder`, `extract`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PagesReport {
    /// Operation subtype (`"rotate"`, `"delete"`, `"reorder"`, `"extract"`).
    pub action: &'static str,
    /// Source PDF file path.
    pub input: PathBuf,
    /// Output PDF file path.
    pub output: PathBuf,
    /// Number of pages in the source PDF.
    pub input_pages: u32,
    /// Number of pages in the output PDF.
    pub output_pages: u32,
    /// 1-based page numbers affected or included by the operation.
    pub pages_affected: Vec<u32>,
    /// Normalized rotation delta (`90`, `180`, `270`) when `action == "rotate"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub degrees: Option<i32>,
}

/// Normalizes a rotation angle in degrees to `90`, `180`, or `270`.
///
/// Accepts `90`, `180`, `270`, `-90` (`270`), `-180` (`180`), and `-270` (`90`).
/// Any other value returns [`SheafError::InvalidSelection`].
///
/// # Examples
///
/// ```
/// use sheaf_core::ops::pages::normalize_rotation_degrees;
///
/// assert_eq!(normalize_rotation_degrees(90).unwrap(), 90);
/// assert_eq!(normalize_rotation_degrees(-90).unwrap(), 270);
/// assert!(normalize_rotation_degrees(45).is_err());
/// ```
pub fn normalize_rotation_degrees(degrees: i32) -> Result<i32, SheafError> {
    match degrees {
        90 | -270 => Ok(90),
        180 | -180 => Ok(180),
        270 | -90 => Ok(270),
        _ => Err(SheafError::InvalidSelection(format!(
            "Invalid rotation angle {degrees}° (supported values: 90, 180, 270, -90)"
        ))),
    }
}

/// Rotates the selected pages in `src` additively by `degrees` (`90`, `180`, `270`, or `-90`)
/// and writes the resulting PDF to `opts.out` (or `{stem}_rotated.pdf`).
///
/// Rotation is **additive**: if a page already has `/Rotate 90` and `rotate_pages` is called
/// with `180`, its new `/Rotate` entry becomes `(90 + 180) % 360 = 270`.
/// If `selection` is `None`, all pages in the document are rotated.
///
/// # Errors
///
/// Returns:
/// - [`SheafError::InvalidSelection`] if `degrees` is not a valid multiple of 90 (`90`, `180`, `270`, `-90`).
/// - [`SheafError::PageOutOfRange`] if `selection` contains a page number outside `1..=total`.
/// - [`SheafError::Encrypted`] if `src` is password-protected.
/// - [`SheafError::OutputExists`] if the output path exists and `!opts.force`.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use sheaf_core::ops::pages::{rotate_pages, PageOpOptions};
/// use sheaf_core::{PageSelection, NoProgress};
///
/// let sel = PageSelection::parse("1-2", 2).unwrap();
/// let opts = PageOpOptions::default();
/// let report = rotate_pages(Path::new("in.pdf"), Some(&sel), 90, &opts, &NoProgress).unwrap();
/// assert_eq!(report.degrees, Some(90));
/// ```
pub fn rotate_pages(
    src: &Path,
    selection: Option<&PageSelection>,
    degrees: i32,
    opts: &PageOpOptions,
    p: &dyn Progress,
) -> Result<PagesReport, SheafError> {
    let delta = normalize_rotation_degrees(degrees)?;
    let (mut doc, pages_map, total_pages) = load_source_pdf(src, p)?;

    let target_pages = match selection {
        Some(sel) => validate_and_dedup_selection(sel, total_pages)?,
        None => (1..=total_pages).collect(),
    };

    let out_path = resolve_output_path(src, opts.out.as_deref(), "rotated");
    ensure_not_overwriting_inputs(&[src.to_path_buf()], &out_path)?;
    check_output_path(&out_path, opts.force)?;

    let total_steps = (target_pages.len() as u64) + 1;
    p.report(0, total_steps);

    if opts.dry_run {
        p.report(total_steps, total_steps);
        return Ok(PagesReport {
            action: "rotate",
            input: src.to_path_buf(),
            output: out_path,
            input_pages: total_pages,
            output_pages: total_pages,
            pages_affected: target_pages,
            degrees: Some(delta),
        });
    }

    for (idx, &page_num) in target_pages.iter().enumerate() {
        if p.is_cancelled() {
            return Err(SheafError::Cancelled);
        }
        let &page_id = pages_map.get(&page_num).ok_or(SheafError::PageOutOfRange {
            page: page_num,
            total: total_pages,
        })?;

        let current_rot = get_effective_page_rotation(&doc, page_id);
        let new_rot = (current_rot + delta).rem_euclid(360);

        let page_dict = doc
            .get_dictionary_mut(page_id)
            .map_err(|e| SheafError::from_lopdf_with_path(e, src))?;
        page_dict.set("Rotate", Object::Integer(i64::from(new_rot)));

        p.report((idx as u64) + 1, total_steps);
    }

    write_atomic(&out_path, opts.force, |file| {
        doc.save_to(file)?;
        Ok(())
    })?;

    p.report(total_steps, total_steps);
    Ok(PagesReport {
        action: "rotate",
        input: src.to_path_buf(),
        output: out_path,
        input_pages: total_pages,
        output_pages: total_pages,
        pages_affected: target_pages,
        degrees: Some(delta),
    })
}

/// Deletes the selected pages from `src` and writes the remaining pages to `opts.out`
/// (or `{stem}_deleted.pdf`).
///
/// # Errors
///
/// Returns:
/// - [`SheafError::PageOutOfRange`] if any page in `selection` is outside `1..=total`.
/// - [`SheafError::InvalidSelection`] if `selection` would delete all pages in the document.
/// - [`SheafError::OutputExists`] if the output path exists and `!opts.force`.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use sheaf_core::ops::pages::{delete_pages, PageOpOptions};
/// use sheaf_core::{PageSelection, NoProgress};
///
/// let sel = PageSelection::parse("2", 3).unwrap();
/// let report = delete_pages(Path::new("in.pdf"), &sel, &PageOpOptions::default(), &NoProgress).unwrap();
/// assert_eq!(report.output_pages, 2);
/// ```
pub fn delete_pages(
    src: &Path,
    selection: &PageSelection,
    opts: &PageOpOptions,
    p: &dyn Progress,
) -> Result<PagesReport, SheafError> {
    let (doc, pages_map, total_pages) = load_source_pdf(src, p)?;
    let to_delete = validate_and_dedup_selection(selection, total_pages)?;
    let delete_set: HashSet<u32> = to_delete.iter().copied().collect();

    let remaining_pages: Vec<u32> = (1..=total_pages)
        .filter(|page| !delete_set.contains(page))
        .collect();

    if remaining_pages.is_empty() {
        return Err(SheafError::InvalidSelection(
            "Cannot delete all pages from a PDF document (at least 1 page must remain)".to_string(),
        ));
    }

    let out_path = resolve_output_path(src, opts.out.as_deref(), "deleted");
    ensure_not_overwriting_inputs(&[src.to_path_buf()], &out_path)?;
    check_output_path(&out_path, opts.force)?;

    p.report(0, 2);

    let output_pages = u32::try_from(remaining_pages.len()).unwrap_or(u32::MAX);
    if opts.dry_run {
        p.report(2, 2);
        return Ok(PagesReport {
            action: "delete",
            input: src.to_path_buf(),
            output: out_path,
            input_pages: total_pages,
            output_pages,
            pages_affected: to_delete,
            degrees: None,
        });
    }

    if p.is_cancelled() {
        return Err(SheafError::Cancelled);
    }

    let mut sub_doc = build_subset_document(&doc, &pages_map, &remaining_pages)?;
    p.report(1, 2);

    write_atomic(&out_path, opts.force, |file| {
        sub_doc.save_to(file)?;
        Ok(())
    })?;

    p.report(2, 2);
    Ok(PagesReport {
        action: "delete",
        input: src.to_path_buf(),
        output: out_path,
        input_pages: total_pages,
        output_pages,
        pages_affected: to_delete,
        degrees: None,
    })
}

/// Reorders (or selects and reorders) pages from `src` according to `order` and writes the
/// resulting PDF to `opts.out` (or `{stem}_reordered.pdf`).
///
/// `order` is a [`PageSelection`] whose ranges/tokens define the exact new page sequence
/// (e.g. `"3,1,2,4-"`).
///
/// # Errors
///
/// Returns:
/// - [`SheafError::EmptySelection`] if `order` is empty.
/// - [`SheafError::PageOutOfRange`] if any page in `order` is outside `1..=total`.
/// - [`SheafError::OutputExists`] if the output path exists and `!opts.force`.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use sheaf_core::ops::pages::{reorder_pages, PageOpOptions};
/// use sheaf_core::{PageSelection, NoProgress};
///
/// let order = PageSelection::parse("3,1,2", 3).unwrap();
/// let report = reorder_pages(Path::new("in.pdf"), &order, &PageOpOptions::default(), &NoProgress).unwrap();
/// assert_eq!(report.output_pages, 3);
/// ```
pub fn reorder_pages(
    src: &Path,
    order: &PageSelection,
    opts: &PageOpOptions,
    p: &dyn Progress,
) -> Result<PagesReport, SheafError> {
    let (doc, pages_map, total_pages) = load_source_pdf(src, p)?;
    let page_sequence = validate_sequence_selection(order, total_pages)?;

    let out_path = resolve_output_path(src, opts.out.as_deref(), "reordered");
    ensure_not_overwriting_inputs(&[src.to_path_buf()], &out_path)?;
    check_output_path(&out_path, opts.force)?;

    p.report(0, 2);

    let output_pages = u32::try_from(page_sequence.len()).unwrap_or(u32::MAX);
    if opts.dry_run {
        p.report(2, 2);
        return Ok(PagesReport {
            action: "reorder",
            input: src.to_path_buf(),
            output: out_path,
            input_pages: total_pages,
            output_pages,
            pages_affected: page_sequence,
            degrees: None,
        });
    }

    if p.is_cancelled() {
        return Err(SheafError::Cancelled);
    }

    let mut sub_doc = build_subset_document(&doc, &pages_map, &page_sequence)?;
    p.report(1, 2);

    write_atomic(&out_path, opts.force, |file| {
        sub_doc.save_to(file)?;
        Ok(())
    })?;

    p.report(2, 2);
    Ok(PagesReport {
        action: "reorder",
        input: src.to_path_buf(),
        output: out_path,
        input_pages: total_pages,
        output_pages,
        pages_affected: page_sequence,
        degrees: None,
    })
}

/// Extracts the selected pages from `src` into a new PDF file at `opts.out`
/// (or `{stem}_extracted.pdf`), preserving the order specified in `selection`.
///
/// # Errors
///
/// Returns:
/// - [`SheafError::EmptySelection`] if `selection` is empty.
/// - [`SheafError::PageOutOfRange`] if any page in `selection` is outside `1..=total`.
/// - [`SheafError::OutputExists`] if the output path exists and `!opts.force`.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use sheaf_core::ops::pages::{extract_pages, PageOpOptions};
/// use sheaf_core::{PageSelection, NoProgress};
///
/// let sel = PageSelection::parse("1,3", 4).unwrap();
/// let report = extract_pages(Path::new("in.pdf"), &sel, &PageOpOptions::default(), &NoProgress).unwrap();
/// assert_eq!(report.output_pages, 2);
/// ```
pub fn extract_pages(
    src: &Path,
    selection: &PageSelection,
    opts: &PageOpOptions,
    p: &dyn Progress,
) -> Result<PagesReport, SheafError> {
    let (doc, pages_map, total_pages) = load_source_pdf(src, p)?;
    let extracted_sequence = validate_sequence_selection(selection, total_pages)?;

    let out_path = resolve_output_path(src, opts.out.as_deref(), "extracted");
    ensure_not_overwriting_inputs(&[src.to_path_buf()], &out_path)?;
    check_output_path(&out_path, opts.force)?;

    p.report(0, 2);

    let output_pages = u32::try_from(extracted_sequence.len()).unwrap_or(u32::MAX);
    if opts.dry_run {
        p.report(2, 2);
        return Ok(PagesReport {
            action: "extract",
            input: src.to_path_buf(),
            output: out_path,
            input_pages: total_pages,
            output_pages,
            pages_affected: extracted_sequence,
            degrees: None,
        });
    }

    if p.is_cancelled() {
        return Err(SheafError::Cancelled);
    }

    let mut sub_doc = build_subset_document(&doc, &pages_map, &extracted_sequence)?;
    p.report(1, 2);

    write_atomic(&out_path, opts.force, |file| {
        sub_doc.save_to(file)?;
        Ok(())
    })?;

    p.report(2, 2);
    Ok(PagesReport {
        action: "extract",
        input: src.to_path_buf(),
        output: out_path,
        input_pages: total_pages,
        output_pages,
        pages_affected: extracted_sequence,
        degrees: None,
    })
}

fn load_source_pdf(
    src: &Path,
    p: &dyn Progress,
) -> Result<(Document, BTreeMap<u32, ObjectId>, u32), SheafError> {
    if p.is_cancelled() {
        return Err(SheafError::Cancelled);
    }
    if !src.exists() {
        return Err(SheafError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found: {}", src.display()),
        )));
    }

    let doc = Document::load(src).map_err(|e| SheafError::from_lopdf_with_path(e, src))?;
    if doc.is_encrypted() {
        return Err(SheafError::Encrypted);
    }

    let pages_map = doc.get_pages();
    if pages_map.is_empty() {
        return Err(SheafError::InvalidPdf(format!(
            "{}: document contains no pages",
            src.display()
        )));
    }
    let total_pages = u32::try_from(pages_map.len()).unwrap_or(u32::MAX);
    Ok((doc, pages_map, total_pages))
}

fn resolve_output_path(src: &Path, configured: Option<&Path>, suffix: &str) -> PathBuf {
    if let Some(out) = configured
        && !out.as_os_str().is_empty()
    {
        return out.to_path_buf();
    }
    let parent = src
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let stem = sanitize_stem(src);
    parent.join(format!("{stem}_{suffix}.pdf"))
}

fn validate_and_dedup_selection(
    selection: &PageSelection,
    total_pages: u32,
) -> Result<Vec<u32>, SheafError> {
    let raw = validate_sequence_selection(selection, total_pages)?;
    let mut seen = HashSet::with_capacity(raw.len());
    let mut deduped = Vec::with_capacity(raw.len());
    for page in raw {
        if seen.insert(page) {
            deduped.push(page);
        }
    }
    Ok(deduped)
}

fn validate_sequence_selection(
    selection: &PageSelection,
    total_pages: u32,
) -> Result<Vec<u32>, SheafError> {
    if selection.ranges().is_empty() {
        return Err(SheafError::EmptySelection);
    }
    let mut pages = Vec::new();
    for r in selection.ranges() {
        if *r.start() == 0 || *r.start() > total_pages {
            return Err(SheafError::PageOutOfRange {
                page: *r.start(),
                total: total_pages,
            });
        }
        if *r.end() == 0 || *r.end() > total_pages {
            return Err(SheafError::PageOutOfRange {
                page: *r.end(),
                total: total_pages,
            });
        }
        if r.start() > r.end() {
            return Err(SheafError::InvalidSelection(format!(
                "Invalid range {}-{}",
                r.start(),
                r.end()
            )));
        }
        pages.extend(r.clone());
    }
    Ok(pages)
}

/// Reads the effective `/Rotate` integer attribute for `page_id`, walking up `/Parent` nodes
/// if `/Rotate` is inherited from an ancestor `/Pages` dictionary.
pub fn get_effective_page_rotation(doc: &Document, page_id: ObjectId) -> i32 {
    let mut current_id = Some(page_id);
    let mut depth = 0usize;
    while let Some(id) = current_id {
        if depth > 32 {
            break;
        }
        depth += 1;
        let Ok(dict) = doc.get_dictionary(id) else {
            break;
        };
        if let Ok(rot_obj) = dict.get_deref(b"Rotate", doc)
            && let Ok(val) = rot_obj.as_i64()
        {
            return (val as i32).rem_euclid(360);
        }
        current_id = dict.get(b"Parent").ok().and_then(|p| p.as_reference().ok());
    }
    0
}
