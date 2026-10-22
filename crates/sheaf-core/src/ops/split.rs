//! PDF split operations (`PageSelection`, `SplitMode`, `split`).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use lopdf::{Dictionary, Document, Object, ObjectId};
use serde::Serialize;

use crate::error::ensure_not_overwriting_inputs;
use crate::fs::{check_output_path, write_atomic};
use crate::load::{LoadOptions, load_pdf};
use crate::{Progress, SheafError};

/// A validated selection of 1-based inclusive page ranges.
///
/// Duplicates and overlapping ranges are allowed and preserved in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageSelection(Vec<RangeInclusive<u32>>);

impl PageSelection {
    /// Parses a comma-separated list of 1-based page ranges against a document with `total` pages.
    ///
    /// Supported tokens:
    /// - `"3"` -> `3..=3`
    /// - `"1-3"` -> `1..=3`
    /// - `"8-"` -> `8..=total`
    /// - `"-3"` -> `1..=3`
    /// - `"1-3,5,8-"` -> multiple ranges
    ///
    /// # Errors
    ///
    /// Returns:
    /// - [`SheafError::EmptySelection`] if `input.trim()` is empty.
    /// - [`SheafError::InvalidSelection`] if a token is malformed (e.g. `"abc"`, `"1--3"`, `""` in `",,"`, `"-"`),
    ///   or if a range is reversed (`"5-3"` where `start > end`).
    /// - [`SheafError::PageOutOfRange`] if any page number is `0` or `> total`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sheaf_core::PageSelection;
    ///
    /// let sel = PageSelection::parse("1-3, 5, 8-", 12).unwrap();
    /// assert_eq!(sel.ranges(), &[1..=3, 5..=5, 8..=12]);
    /// ```
    pub fn parse(input: &str, total: u32) -> Result<Self, SheafError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(SheafError::EmptySelection);
        }

        let mut ranges = Vec::new();
        for raw_token in trimmed.split(',') {
            let token = raw_token.trim();
            if token.is_empty() {
                return Err(SheafError::InvalidSelection(
                    "Empty range token in page selection".to_string(),
                ));
            }

            if let Some((left, right)) = token.split_once('-') {
                // Reject multiple hyphens like "1--3" or "1-2-3"
                if right.contains('-') {
                    return Err(SheafError::InvalidSelection(format!(
                        "Invalid range format '{token}'"
                    )));
                }

                let left = left.trim();
                let right = right.trim();

                if left.is_empty() && right.is_empty() {
                    return Err(SheafError::InvalidSelection(
                        "Invalid range '-'".to_string(),
                    ));
                }

                let start = if left.is_empty() {
                    check_page_bounds(1, total)?;
                    1
                } else {
                    let p = parse_page_num(left, token)?;
                    check_page_bounds(p, total)?;
                    p
                };

                let end = if right.is_empty() {
                    check_page_bounds(total, total)?;
                    total
                } else {
                    let p = parse_page_num(right, token)?;
                    check_page_bounds(p, total)?;
                    p
                };

                if start > end {
                    return Err(SheafError::InvalidSelection(format!(
                        "Start page ({start}) must not be greater than end page ({end}) ('{token}')"
                    )));
                }

                ranges.push(start..=end);
            } else {
                let p = parse_page_num(token, token)?;
                check_page_bounds(p, total)?;
                ranges.push(p..=p);
            }
        }

        Ok(Self(ranges))
    }

    /// Returns the parsed slice of 1-based inclusive page ranges.
    ///
    /// # Examples
    ///
    /// ```
    /// use sheaf_core::PageSelection;
    ///
    /// let sel = PageSelection::parse("2-4", 10).unwrap();
    /// assert_eq!(sel.ranges(), &[2..=4]);
    /// ```
    pub fn ranges(&self) -> &[RangeInclusive<u32>] {
        &self.0
    }

    /// Expands all ranges into a flat `Vec<u32>` of 1-based page numbers in the exact order specified.
    ///
    /// # Examples
    ///
    /// ```
    /// use sheaf_core::PageSelection;
    ///
    /// let sel = PageSelection::parse("1-2, 5, 2", 6).unwrap();
    /// assert_eq!(sel.to_page_numbers(), vec![1, 2, 5, 2]);
    /// ```
    pub fn to_page_numbers(&self) -> Vec<u32> {
        let mut pages = Vec::new();
        for r in &self.0 {
            pages.extend(r.clone());
        }
        pages
    }
}

fn parse_page_num(s: &str, full_token: &str) -> Result<u32, SheafError> {
    if !s.chars().all(|c| c.is_ascii_digit()) {
        return Err(SheafError::InvalidSelection(format!(
            "Invalid page number in '{full_token}'"
        )));
    }
    s.parse::<u32>()
        .map_err(|_| SheafError::InvalidSelection(format!("Invalid page number in '{full_token}'")))
}

fn check_page_bounds(page: u32, total: u32) -> Result<(), SheafError> {
    if page == 0 || page > total {
        return Err(SheafError::PageOutOfRange { page, total });
    }
    Ok(())
}

/// Strategy for splitting a PDF document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplitMode {
    /// Split by explicit page ranges (either one file per range or combined into one file).
    Ranges {
        /// Parsed 1-based page ranges.
        selection: PageSelection,
        /// Combine all selected ranges into a single output file (`{stem}_selection.pdf`).
        combine: bool,
    },
    /// Split into chunks of `N` pages (`N >= 1`).
    EveryN(u32),
    /// Split at bookmarks (outlines) of the given 1-based hierarchy level (`level >= 1`).
    ByBookmarks {
        /// 1-based bookmark hierarchy level (`1` = top-level chapters).
        level: u8,
    },
}

impl SplitMode {
    /// Stable mode name used in [`SplitReport`].
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ranges { .. } => "ranges",
            Self::EveryN(_) => "every_n",
            Self::ByBookmarks { .. } => "bookmarks",
        }
    }
}

/// Options for [`split`].
#[derive(Debug, Clone)]
pub struct SplitOptions {
    /// Loading and decryption options for input PDF.
    pub load: LoadOptions,
    /// Split mode.
    pub mode: SplitMode,
    /// Target directory for generated PDF files.
    pub out_dir: PathBuf,
    /// Overwrite existing output files if `true`.
    pub force: bool,
    /// Validate inputs and compute output plan without writing any files.
    pub dry_run: bool,
}

/// Metadata about a single generated split PDF file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SplitOutput {
    /// Output file path.
    pub path: PathBuf,
    /// Number of pages in this output PDF.
    pub pages: u32,
    /// 1-based `(start_page, end_page)` in the source PDF (`None` when `combine` joins non-contiguous ranges).
    pub page_range: Option<(u32, u32)>,
    /// Bookmark section title when split via [`SplitMode::ByBookmarks`].
    pub title: Option<String>,
}

/// Report returned by [`split`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SplitReport {
    /// Input PDF path.
    pub input: PathBuf,
    /// Number of pages in the input PDF.
    pub input_pages: u32,
    /// Split mode identifier (`"ranges"`, `"every_n"`, or `"bookmarks"`).
    pub mode: &'static str,
    /// Generated (or planned, in `--dry-run` mode) output files.
    pub outputs: Vec<SplitOutput>,
    /// Non-fatal warnings produced during splitting (e.g. skipped bookmarks without valid target).
    #[serde(skip)]
    pub warnings: Vec<String>,
    /// Whether the input PDF was originally encrypted.
    #[serde(skip)]
    pub was_encrypted: bool,
}

struct PlannedOutput {
    output: SplitOutput,
    page_numbers: Vec<u32>,
}

/// Splits a PDF document (`src`) into one or more output PDFs according to [`SplitOptions`].
///
/// The input PDF is loaded once. Outlines in the generated split outputs are removed to avoid
/// dangling cross-page references, while the document metadata (`Info` dictionary) is preserved.
/// All output paths are checked for collisions (`OutputExists`) before the first file is written.
///
/// # Errors
///
/// Returns:
/// - [`SheafError::Io`] if `src` cannot be read or output files cannot be written.
/// - [`SheafError::Encrypted`] if `src` is encrypted.
/// - [`SheafError::InvalidPdf`] if `src` is corrupted.
/// - [`SheafError::InvalidSelection`] if `EveryN(0)` or `ByBookmarks { level: 0 }` is given.
/// - [`SheafError::PageOutOfRange`] if a range exceeds the document's page count.
/// - [`SheafError::Unsupported`] if `ByBookmarks` finds no bookmarks on the requested level.
/// - [`SheafError::OutputExists`] if any target file already exists and `!opts.force`.
/// - [`SheafError::Cancelled`] if cancelled via `p`.
///
/// # Examples
///
/// ```no_run
/// use std::path::{Path, PathBuf};
/// use sheaf_core::{LoadOptions, NoProgress, PageSelection, SplitMode, SplitOptions, split};
///
/// let sel = PageSelection::parse("1-2", 2).unwrap();
/// let opts = SplitOptions {
///     load: LoadOptions::none(),
///     mode: SplitMode::Ranges { selection: sel, combine: false },
///     out_dir: PathBuf::from("out"),
///     force: false,
///     dry_run: false,
/// };
/// let report = split(Path::new("input.pdf"), &opts, &NoProgress).unwrap();
/// assert_eq!(report.outputs.len(), 1);
/// ```
pub fn split(src: &Path, opts: &SplitOptions, p: &dyn Progress) -> Result<SplitReport, SheafError> {
    if p.is_cancelled() {
        return Err(SheafError::Cancelled);
    }

    let loaded = load_pdf(src, &opts.load)?;
    let was_encrypted = loaded.was_encrypted;
    let doc = loaded.doc;

    let pages_map = doc.get_pages();
    if pages_map.is_empty() {
        return Err(SheafError::InvalidPdf(format!(
            "{}: document contains no pages",
            src.display()
        )));
    }
    let total_pages = u32::try_from(pages_map.len()).unwrap_or(u32::MAX);

    let out_dir = resolve_out_dir(src, &opts.out_dir);
    let stem = sanitize_stem(src);

    let mut warnings = Vec::new();
    let planned = plan_outputs(
        &doc,
        total_pages,
        &stem,
        &out_dir,
        &opts.mode,
        &mut warnings,
    )?;

    // Pre-check all output paths before writing a single file so we never leave partial output
    let src_vec = vec![src.to_path_buf()];
    for item in &planned {
        ensure_not_overwriting_inputs(&src_vec, &item.output.path)?;
        check_output_path(&item.output.path, opts.force)?;
    }

    let total_steps = planned.len() as u64;
    p.report(0, total_steps);

    if opts.dry_run {
        p.report(total_steps, total_steps);
        return Ok(SplitReport {
            input: src.to_path_buf(),
            input_pages: total_pages,
            mode: opts.mode.as_str(),
            outputs: planned.into_iter().map(|item| item.output).collect(),
            warnings,
            was_encrypted,
        });
    }

    let mut outputs = Vec::with_capacity(planned.len());
    for (idx, item) in planned.into_iter().enumerate() {
        if p.is_cancelled() {
            return Err(SheafError::Cancelled);
        }

        let mut sub_doc = build_subset_document(&doc, &pages_map, &item.page_numbers)?;
        write_atomic(&item.output.path, opts.force, |file| {
            sub_doc.save_to(file)?;
            Ok(())
        })?;

        outputs.push(item.output);
        p.report((idx as u64) + 1, total_steps);
    }

    Ok(SplitReport {
        input: src.to_path_buf(),
        input_pages: total_pages,
        mode: opts.mode.as_str(),
        outputs,
        warnings,
        was_encrypted,
    })
}

fn resolve_out_dir(src: &Path, configured: &Path) -> PathBuf {
    if configured.as_os_str().is_empty() {
        src.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    } else {
        configured.to_path_buf()
    }
}

pub(crate) fn sanitize_stem(src: &Path) -> String {
    let raw = src
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    let cleaned: String = raw
        .chars()
        .filter(|&c| c != '/' && c != '\\' && c != '\0')
        .collect();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        "document".to_string()
    } else {
        cleaned
    }
}

fn plan_outputs(
    doc: &Document,
    total_pages: u32,
    stem: &str,
    out_dir: &Path,
    mode: &SplitMode,
    warnings: &mut Vec<String>,
) -> Result<Vec<PlannedOutput>, SheafError> {
    match mode {
        SplitMode::Ranges { selection, combine } => {
            if selection.ranges().is_empty() {
                return Err(SheafError::EmptySelection);
            }
            for r in selection.ranges() {
                check_page_bounds(*r.start(), total_pages)?;
                check_page_bounds(*r.end(), total_pages)?;
                if r.start() > r.end() {
                    return Err(SheafError::InvalidSelection(format!(
                        "Invalid range {}-{}",
                        r.start(),
                        r.end()
                    )));
                }
            }

            if *combine {
                let mut page_numbers = Vec::new();
                for r in selection.ranges() {
                    page_numbers.extend(r.clone());
                }
                let page_range = if selection.ranges().len() == 1 {
                    let r = &selection.ranges()[0];
                    Some((*r.start(), *r.end()))
                } else {
                    None
                };
                let path = out_dir.join(format!("{stem}_selection.pdf"));
                Ok(vec![PlannedOutput {
                    output: SplitOutput {
                        path,
                        pages: u32::try_from(page_numbers.len()).unwrap_or(u32::MAX),
                        page_range,
                        title: None,
                    },
                    page_numbers,
                }])
            } else {
                let mut planned = Vec::with_capacity(selection.ranges().len());
                let mut name_counts: HashMap<String, usize> = HashMap::new();

                for r in selection.ranges() {
                    let start = *r.start();
                    let end = *r.end();
                    let base_name = if start == end {
                        format!("{stem}_p{start}")
                    } else {
                        format!("{stem}_p{start}-{end}")
                    };
                    let count = name_counts.entry(base_name.clone()).or_insert(0);
                    *count += 1;
                    let file_name = if *count == 1 {
                        format!("{base_name}.pdf")
                    } else {
                        format!("{base_name}_{count}.pdf")
                    };

                    let page_numbers: Vec<u32> = (start..=end).collect();
                    planned.push(PlannedOutput {
                        output: SplitOutput {
                            path: out_dir.join(file_name),
                            pages: end - start + 1,
                            page_range: Some((start, end)),
                            title: None,
                        },
                        page_numbers,
                    });
                }
                Ok(planned)
            }
        }
        SplitMode::EveryN(n) => {
            if *n == 0 {
                return Err(SheafError::InvalidSelection(
                    "Chunk size N for --every must be at least 1".to_string(),
                ));
            }
            let num_parts = total_pages.div_ceil(*n);
            let width = if num_parts > 99 { 3 } else { 2 };

            let mut planned = Vec::with_capacity(num_parts as usize);
            let mut start = 1u32;
            let mut part_idx = 1u32;

            while start <= total_pages {
                let end = (start + *n - 1).min(total_pages);
                let file_name = format!("{stem}_part{part_idx:0width$}.pdf");
                let page_numbers: Vec<u32> = (start..=end).collect();
                planned.push(PlannedOutput {
                    output: SplitOutput {
                        path: out_dir.join(file_name),
                        pages: end - start + 1,
                        page_range: Some((start, end)),
                        title: None,
                    },
                    page_numbers,
                });
                start = end + 1;
                part_idx += 1;
            }
            Ok(planned)
        }
        SplitMode::ByBookmarks { level } => {
            if *level == 0 {
                return Err(SheafError::InvalidSelection(
                    "Bookmark level must be at least 1".to_string(),
                ));
            }
            plan_by_bookmarks(doc, total_pages, stem, out_dir, *level, warnings)
        }
    }
}

#[derive(Debug)]
struct BookmarkTarget {
    title: String,
    level: u8,
    page: Option<u32>,
}

fn plan_by_bookmarks(
    doc: &Document,
    total_pages: u32,
    stem: &str,
    out_dir: &Path,
    target_level: u8,
    warnings: &mut Vec<String>,
) -> Result<Vec<PlannedOutput>, SheafError> {
    let entries = collect_outline_targets(doc)?;
    let level_entries: Vec<BookmarkTarget> = entries
        .into_iter()
        .filter(|e| e.level == target_level)
        .collect();

    let mut valid_bookmarks: Vec<(String, u32)> = Vec::new();
    for entry in level_entries {
        match entry.page {
            Some(p) if (1..=total_pages).contains(&p) => {
                valid_bookmarks.push((entry.title, p));
            }
            _ => {
                warnings.push(format!(
                    "Skipped bookmark without valid destination: {}",
                    entry.title
                ));
            }
        }
    }

    if valid_bookmarks.is_empty() {
        return Err(SheafError::Unsupported(format!(
            "Document has no bookmarks at level {target_level}"
        )));
    }

    // Deduplicate or ignore out-of-order bookmarks that point earlier than the previous bookmark
    let mut sections: Vec<(String, u32)> = Vec::new();
    for (title, page) in valid_bookmarks {
        if let Some((_, last_page)) = sections.last()
            && page < *last_page
        {
            warnings.push(format!(
                "Skipped bookmark with backward page target ({page}): {title}"
            ));
            continue;
        }
        sections.push((title, page));
    }

    let total_parts = sections.len() + usize::from(sections[0].1 > 1);
    let width = if total_parts > 99 { 3 } else { 2 };

    let mut planned = Vec::with_capacity(total_parts);
    let mut slug_counts: HashMap<String, usize> = HashMap::new();

    if sections[0].1 > 1 {
        let end = sections[0].1 - 1;
        let file_name = format!("{stem}_{num:0width$}_front.pdf", num = 0);
        planned.push(PlannedOutput {
            output: SplitOutput {
                path: out_dir.join(file_name),
                pages: end,
                page_range: Some((1, end)),
                title: Some("front".to_string()),
            },
            page_numbers: (1..=end).collect(),
        });
    }

    for (idx, (title, start_page)) in sections.iter().enumerate() {
        let end_page = if let Some((_, next_start)) = sections.get(idx + 1) {
            if *next_start > *start_page {
                *next_start - 1
            } else {
                *start_page
            }
        } else {
            total_pages
        };

        let base_slug = slugify(title);
        let count = slug_counts.entry(base_slug.clone()).or_insert(0);
        *count += 1;
        let final_slug = if *count == 1 {
            base_slug
        } else {
            format!("{base_slug}-{count}")
        };

        let num = idx + 1;
        let file_name = format!("{stem}_{num:0width$}_{final_slug}.pdf");
        let page_numbers: Vec<u32> = (*start_page..=end_page).collect();

        planned.push(PlannedOutput {
            output: SplitOutput {
                path: out_dir.join(file_name),
                pages: end_page - *start_page + 1,
                page_range: Some((*start_page, end_page)),
                title: Some(title.clone()),
            },
            page_numbers,
        });
    }

    Ok(planned)
}

/// Walks the `/Outlines` tree of `doc` and extracts bookmark titles, hierarchy levels (1-based),
/// and resolved 1-based page numbers (`None` if the bookmark has no valid page destination).
fn collect_outline_targets(doc: &Document) -> Result<Vec<BookmarkTarget>, SheafError> {
    let Ok(catalog) = doc.catalog() else {
        return Ok(Vec::new());
    };
    let Ok(outlines_dict) = doc.get_dict_in_dict(catalog, b"Outlines") else {
        return Ok(Vec::new());
    };
    let Ok(first_obj) = outlines_dict.get(b"First") else {
        return Ok(Vec::new());
    };

    let page_id_to_num: BTreeMap<ObjectId, u32> = doc
        .get_pages()
        .into_iter()
        .map(|(num, id)| (id, num))
        .collect();

    let mut results = Vec::new();
    let mut visited = HashSet::new();
    walk_outline_nodes(
        doc,
        first_obj,
        1,
        &page_id_to_num,
        &mut visited,
        &mut results,
    );
    Ok(results)
}

fn walk_outline_nodes(
    doc: &Document,
    start_obj: &Object,
    level: u8,
    page_id_to_num: &BTreeMap<ObjectId, u32>,
    visited: &mut HashSet<ObjectId>,
    out: &mut Vec<BookmarkTarget>,
) {
    if level > 32 {
        return;
    }

    let mut current_obj = Some(start_obj.clone());
    while let Some(obj) = current_obj.take() {
        let node_dict = match &obj {
            Object::Reference(id) => {
                if !visited.insert(*id) {
                    break;
                }
                let Ok(d) = doc.get_dictionary(*id) else {
                    break;
                };
                d
            }
            Object::Dictionary(d) => d,
            _ => break,
        };

        let title = extract_outline_title(doc, node_dict).unwrap_or_else(|| "untitled".to_string());
        let page = resolve_outline_page(doc, node_dict, page_id_to_num);

        out.push(BookmarkTarget { title, level, page });

        if let Ok(first_child) = node_dict.get(b"First") {
            walk_outline_nodes(
                doc,
                first_child,
                level.saturating_add(1),
                page_id_to_num,
                visited,
                out,
            );
        }

        if let Ok(next_obj) = node_dict.get(b"Next") {
            current_obj = Some(next_obj.clone());
        }
    }
}

fn extract_outline_title(doc: &Document, node: &Dictionary) -> Option<String> {
    let title_obj = node.get(b"Title").ok()?;
    let resolved = match title_obj {
        Object::Reference(id) => doc.get_object(*id).ok()?,
        other => other,
    };
    if let Ok(bytes) = resolved.as_str()
        && !bytes.starts_with(b"\xFE\xFF")
        && !bytes.starts_with(b"\xFF\xFE")
        && !bytes.starts_with(b"\xEF\xBB\xBF")
        && let Ok(utf8) = std::str::from_utf8(bytes)
    {
        return Some(utf8.to_string());
    }
    lopdf::decode_text_string(resolved).ok()
}

fn resolve_outline_page(
    doc: &Document,
    node: &Dictionary,
    page_id_to_num: &BTreeMap<ObjectId, u32>,
) -> Option<u32> {
    if let Ok(dest) = node.get(b"Dest") {
        return resolve_dest_object(doc, dest, page_id_to_num);
    }
    let action_dict = doc.get_dict_in_dict(node, b"A").ok()?;
    let cmd = action_dict.get(b"S").ok()?.as_name().ok()?;
    if cmd != b"GoTo" {
        return None;
    }
    let d_obj = action_dict.get(b"D").ok()?;
    resolve_dest_object(doc, d_obj, page_id_to_num)
}

fn resolve_dest_object(
    doc: &Document,
    dest: &Object,
    page_id_to_num: &BTreeMap<ObjectId, u32>,
) -> Option<u32> {
    match dest {
        Object::Array(arr) => {
            let page_id = arr.first()?.as_reference().ok()?;
            page_id_to_num.get(&page_id).copied()
        }
        Object::Reference(id) => {
            if let Some(&num) = page_id_to_num.get(id) {
                return Some(num);
            }
            let target = doc.get_object(*id).ok()?;
            resolve_dest_object(doc, target, page_id_to_num)
        }
        _ => None,
    }
}

/// Transliterates and sanitizes a bookmark title into a safe filename slug:
/// lowercase, only `a-z0-9-`, max 40 chars, German umlauts transliterated (`ä` -> `ae`, etc.).
pub fn slugify(title: &str) -> String {
    let mut buf = String::with_capacity(title.len());
    for ch in title.chars() {
        match ch {
            'ä' | 'Ä' => buf.push_str("ae"),
            'ö' | 'Ö' => buf.push_str("oe"),
            'ü' | 'Ü' => buf.push_str("ue"),
            'ß' | 'ẞ' => buf.push_str("ss"),
            c if c.is_ascii_alphanumeric() => buf.push(c.to_ascii_lowercase()),
            _ => buf.push('-'),
        }
    }

    // Collapse consecutive hyphens and trim leading/trailing hyphens
    let mut collapsed = String::with_capacity(buf.len());
    let mut prev_hyphen = true;
    for ch in buf.chars() {
        if ch == '-' {
            if !prev_hyphen {
                collapsed.push('-');
                prev_hyphen = true;
            }
        } else {
            collapsed.push(ch);
            prev_hyphen = false;
        }
    }

    while collapsed.ends_with('-') {
        collapsed.pop();
    }

    if collapsed.len() > 40 {
        collapsed.truncate(40);
        while collapsed.ends_with('-') {
            collapsed.pop();
        }
    }

    if collapsed.is_empty() {
        "section".to_string()
    } else {
        collapsed
    }
}

/// Builds a standalone PDF document containing the 1-based `selected_pages` from `base_doc`
/// in the exact order specified (supporting duplicates and arbitrary ordering).
pub(crate) fn build_subset_document(
    base_doc: &Document,
    pages_map: &BTreeMap<u32, ObjectId>,
    selected_pages: &[u32],
) -> Result<Document, SheafError> {
    let mut doc = base_doc.clone();

    // Remove outlines to prevent dangling references to deleted pages
    let _ = doc.delete_outlines();
    if let Ok(cat) = doc.catalog_mut() {
        cat.remove(b"Outlines");
        cat.remove(b"Dests");
        cat.remove(b"Names");
    }

    let root_pages_id = doc
        .catalog()
        .and_then(|cat| cat.get(b"Pages"))
        .and_then(Object::as_reference)
        .map_err(SheafError::from_lopdf)?;

    // Check if all selected pages are unique and can simply use `delete_pages`,
    // or if we need to rebuild the Kids array to preserve order and duplicates.
    let mut seen = HashSet::with_capacity(selected_pages.len());
    let mut new_kids = Vec::with_capacity(selected_pages.len());

    for &page_num in selected_pages {
        let &orig_page_id = pages_map.get(&page_num).ok_or(SheafError::PageOutOfRange {
            page: page_num,
            total: u32::try_from(pages_map.len()).unwrap_or(u32::MAX),
        })?;

        let page_id = if seen.insert(orig_page_id) {
            orig_page_id
        } else {
            // Duplicate page in selection: clone the Page dictionary into a new object ID
            let page_obj = doc
                .get_object(orig_page_id)
                .map_err(SheafError::from_lopdf)?
                .clone();
            doc.add_object(page_obj)
        };

        if let Ok(page_dict) = doc.get_dictionary_mut(page_id) {
            page_dict.set("Parent", root_pages_id);
        }
        new_kids.push(Object::Reference(page_id));
    }

    // Remove any unselected page objects
    for &page_id in pages_map.values() {
        if !seen.contains(&page_id) {
            doc.objects.remove(&page_id);
        }
    }

    // Update the root Pages dictionary with the exact ordered Kids list
    if let Ok(pages_dict) = doc.get_dictionary_mut(root_pages_id) {
        pages_dict.set("Kids", Object::Array(new_kids.clone()));
        pages_dict.set("Count", Object::Integer(new_kids.len() as i64));
        pages_dict.remove(b"Parent");
    }

    doc.prune_objects();
    doc.renumber_objects();
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_transliterates_and_truncates() {
        assert_eq!(
            slugify("1. Einführung & Überblick!"),
            "1-einfuehrung-ueberblick"
        );
        assert_eq!(slugify("Größe / Maßstab"), "groesse-massstab");
        assert_eq!(slugify("../../../etc/passwd"), "etc-passwd");
        assert_eq!(slugify("   ---   "), "section");
        let long = "a".repeat(60);
        assert_eq!(slugify(&long).len(), 40);
    }
}
