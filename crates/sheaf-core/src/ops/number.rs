//! PDF page numbering operations (`NumberPosition`, `NumberOptions`, `NumberReport`, `number`, `format_page_number`).

use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::ensure_not_overwriting_inputs;
use crate::fs::{check_output_path, write_atomic};
use crate::load::{LoadOptions, load_pdf_from_bytes};
use crate::ops::overlay::{
    OverlayPosition, PageOverlaySpec, apply_overlay_to_page, create_overlay_shared_objects,
    encode_win_ansi, parse_overlay_color,
};
use crate::ops::split::{PageSelection, sanitize_stem};
use crate::{Progress, SheafError};

/// Placement preset for PDF page numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NumberPosition {
    /// Bottom-center of the visual page (default).
    #[default]
    BottomCenter,
    /// Bottom-right corner of the visual page.
    BottomRight,
    /// Bottom-left corner of the visual page.
    BottomLeft,
    /// Top-center of the visual page.
    TopCenter,
    /// Top-right corner of the visual page.
    TopRight,
    /// Top-left corner of the visual page.
    TopLeft,
}

impl NumberPosition {
    /// Parses a page-number position (`"bottom-center"`, `"bottom-right"`, `"bottom-left"`,
    /// `"top-center"`, `"top-right"`, `"top-left"`).
    ///
    /// # Examples
    ///
    /// ```
    /// use sheaf_core::NumberPosition;
    ///
    /// assert_eq!(NumberPosition::parse("bottom-center").unwrap(), NumberPosition::BottomCenter);
    /// assert_eq!(NumberPosition::parse("top-right").unwrap(), NumberPosition::TopRight);
    /// assert!(NumberPosition::parse("diagonal").is_err());
    /// ```
    pub fn parse(input: &str) -> Result<Self, SheafError> {
        match input.trim().to_ascii_lowercase().as_str() {
            "bottom-center" => Ok(Self::BottomCenter),
            "bottom-right" => Ok(Self::BottomRight),
            "bottom-left" => Ok(Self::BottomLeft),
            "top-center" => Ok(Self::TopCenter),
            "top-right" => Ok(Self::TopRight),
            "top-left" => Ok(Self::TopLeft),
            other => Err(SheafError::InvalidSelection(format!(
                "Invalid page number position '{other}' (expected: bottom-center, bottom-right, bottom-left, top-center, top-right, or top-left)"
            ))),
        }
    }

    /// Stable kebab-case identifier.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BottomCenter => "bottom-center",
            Self::BottomRight => "bottom-right",
            Self::BottomLeft => "bottom-left",
            Self::TopCenter => "top-center",
            Self::TopRight => "top-right",
            Self::TopLeft => "top-left",
        }
    }

    /// Converts this page-number position into the shared [`OverlayPosition`].
    pub fn to_overlay_position(self) -> OverlayPosition {
        match self {
            Self::BottomCenter => OverlayPosition::BottomCenter,
            Self::BottomRight => OverlayPosition::BottomRight,
            Self::BottomLeft => OverlayPosition::BottomLeft,
            Self::TopCenter => OverlayPosition::TopCenter,
            Self::TopRight => OverlayPosition::TopRight,
            Self::TopLeft => OverlayPosition::TopLeft,
        }
    }
}

/// Options for [`number`].
#[derive(Debug, Clone)]
pub struct NumberOptions {
    /// Loading and decryption options for input PDF.
    pub load: LoadOptions,
    /// Format template supporting `{n}`, `{total}`, `{page}`, `{pages}` (default `"{n} / {total}"`).
    pub format: String,
    /// Placement on the page (default `BottomCenter`).
    pub position: NumberPosition,
    /// Starting number for `{n}` on the first selected page (default `1`).
    pub start: u32,
    /// Font size in PDF points (`4.0..=72.0`, default `10.0`).
    pub font_size: f32,
    /// Opacity (`0.05..=1.0`, default `1.0`).
    pub opacity: f32,
    /// Color as `#RRGGBB` hex or named preset (`gray`, `red`, `blue`, `black`; default `#444444`).
    pub color: String,
    /// Margin inset from page edges in PDF points (`0.0..=200.0`, default `28.0`).
    pub margin: f32,
    /// Optional 1-based page selection (`None` numbers all pages).
    pub pages: Option<String>,
    /// Target output PDF path (defaults to `{stem}_numbered.pdf`).
    pub out: Option<PathBuf>,
    /// Overwrite existing output file if `true`.
    pub force: bool,
    /// Compute page-numbering plan without writing the output file if `true`.
    pub dry_run: bool,
}

impl Default for NumberOptions {
    fn default() -> Self {
        Self {
            load: LoadOptions::default(),
            format: "{n}".to_string(),
            position: NumberPosition::BottomCenter,
            start: 1,
            font_size: 10.0,
            opacity: 1.0,
            color: "#444444".to_string(),
            margin: 28.0,
            pages: None,
            out: None,
            force: false,
            dry_run: false,
        }
    }
}

/// Report returned by [`number`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NumberReport {
    /// Input PDF file path.
    pub input: PathBuf,
    /// Output PDF file path.
    pub output: PathBuf,
    /// Format template applied.
    pub format: String,
    /// Position preset name.
    pub position: &'static str,
    /// Starting sequence number `{n}`.
    pub start: u32,
    /// Font size in PDF points.
    pub font_size: f32,
    /// Opacity (`0.05..=1.0`).
    pub opacity: f32,
    /// Normalized `#RRGGBB` color string.
    pub color: String,
    /// Total pages in the document.
    pub total_pages: u32,
    /// 1-based page numbers where numbering was applied.
    pub pages_numbered: Vec<u32>,
    /// Indicates whether `--dry-run` was active.
    pub dry_run: bool,
    /// Whether the input PDF was originally encrypted.
    #[serde(skip)]
    pub was_encrypted: bool,
}

/// Expands a page-number template containing `{n}`, `{total}`, `{page}`, and `{pages}`.
///
/// - `{n}`: sequence number for the current selected page (`start + index`).
/// - `{total}`: count of selected pages being numbered.
/// - `{page}`: physical 1-based page number in the document.
/// - `{pages}`: total physical pages in the document.
///
/// # Examples
///
/// ```
/// use sheaf_core::ops::number::format_page_number;
///
/// assert_eq!(format_page_number("Page {n} of {total} ({page}/{pages})", 1, 5, 2, 6).unwrap(), "Page 1 of 5 (2/6)");
/// assert!(format_page_number("Page {unknown}", 1, 5, 1, 5).is_err());
/// ```
pub fn format_page_number(
    template: &str,
    n: u32,
    total: u32,
    page: u32,
    pages: u32,
) -> Result<String, SheafError> {
    if template.trim().is_empty() {
        return Err(SheafError::InvalidSelection(
            "Page number format template cannot be empty".to_string(),
        ));
    }

    let mut out = String::with_capacity(template.len() + 16);
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            if chars.peek() == Some(&'{') {
                let _ = chars.next();
                out.push('{');
                continue;
            }
            let mut token = String::new();
            let mut closed = false;
            for inner in chars.by_ref() {
                if inner == '}' {
                    closed = true;
                    break;
                }
                token.push(inner);
            }
            if !closed {
                return Err(SheafError::InvalidSelection(format!(
                    "Unclosed '{{' in page number format '{template}'"
                )));
            }
            match token.as_str() {
                "n" => out.push_str(&n.to_string()),
                "total" => out.push_str(&total.to_string()),
                "page" => out.push_str(&page.to_string()),
                "pages" => out.push_str(&pages.to_string()),
                other => {
                    return Err(SheafError::InvalidSelection(format!(
                        "Unknown placeholder '{{{other}}}' in format '{template}' (supported: {{n}}, {{total}}, {{page}}, {{pages}})"
                    )));
                }
            }
        } else if ch == '}' {
            if chars.peek() == Some(&'}') {
                let _ = chars.next();
                out.push('}');
            } else {
                return Err(SheafError::InvalidSelection(format!(
                    "Unmatched '}}' in page number format '{template}'"
                )));
            }
        } else {
            out.push(ch);
        }
    }

    if out.trim().is_empty() {
        return Err(SheafError::InvalidSelection(
            "Expanded page number text cannot be empty".to_string(),
        ));
    }

    let _ = encode_win_ansi(&out)?;
    Ok(out)
}

/// Stamps vector page numbers onto all or selected pages of a PDF (`src`) using the shared
/// [`crate::ops::overlay`] engine.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use sheaf_core::{NoProgress, NumberOptions, number};
///
/// let opts = NumberOptions::default();
/// let report = number(Path::new("input.pdf"), &opts, &NoProgress).unwrap();
/// println!("Numbered {} pages", report.pages_numbered.len());
/// ```
pub fn number(
    src: &Path,
    opts: &NumberOptions,
    p: &dyn Progress,
) -> Result<NumberReport, SheafError> {
    if opts.start == 0 {
        return Err(SheafError::InvalidSelection(
            "Starting page number (--start) must be >= 1".to_string(),
        ));
    }
    if !(4.0..=72.0).contains(&opts.font_size) {
        return Err(SheafError::InvalidSelection(format!(
            "Page number font size must be between 4 and 72 pt (got {})",
            opts.font_size
        )));
    }
    if !(0.05..=1.0).contains(&opts.opacity) {
        return Err(SheafError::InvalidSelection(format!(
            "Page number opacity must be between 0.05 and 1.0 (got {})",
            opts.opacity
        )));
    }
    if !(0.0..=200.0).contains(&opts.margin) {
        return Err(SheafError::InvalidSelection(format!(
            "Page number margin must be between 0 and 200 pt (got {})",
            opts.margin
        )));
    }

    // Validate template syntax and WinAnsi compatibility up front
    let _ = format_page_number(&opts.format, opts.start, 1, 1, 1)?;
    let ((r, g, b), canonical_color) = parse_overlay_color(&opts.color)?;

    if !src.exists() {
        return Err(SheafError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found: {}", src.display()),
        )));
    }

    let out_path = match &opts.out {
        Some(path) => path.clone(),
        None => {
            let parent = src.parent().unwrap_or_else(|| Path::new("."));
            let stem = sanitize_stem(src);
            parent.join(format!("{stem}_numbered.pdf"))
        }
    };
    ensure_not_overwriting_inputs(&[src.to_path_buf()], &out_path)?;
    if !opts.dry_run {
        check_output_path(&out_path, opts.force)?;
    }

    let raw_bytes = fs::read(src)?;
    let loaded = load_pdf_from_bytes(&raw_bytes, src, &opts.load)?;
    let was_encrypted = loaded.was_encrypted;
    let mut doc = loaded.doc;

    let pages_map = doc.get_pages();
    let total_pages = pages_map.len() as u32;
    if total_pages == 0 {
        return Err(SheafError::InvalidPdf(format!(
            "{}: Document contains 0 pages",
            src.display()
        )));
    }

    let target_pages: Vec<u32> = match &opts.pages {
        Some(spec) if !spec.trim().is_empty() => {
            PageSelection::parse(spec, total_pages)?.to_page_numbers()
        }
        _ => (1..=total_pages).collect(),
    };

    if !opts.dry_run {
        let (font_id, gs_id) = create_overlay_shared_objects(&mut doc, "Helvetica", opts.opacity);
        let selected_total = target_pages.len() as u32;
        let total_steps = target_pages.len() as u64;

        for (idx, &page_num) in target_pages.iter().enumerate() {
            if p.is_cancelled() {
                return Err(SheafError::Cancelled);
            }
            p.report(idx as u64, total_steps);

            let seq_n = opts.start.saturating_add(idx as u32);
            let text =
                format_page_number(&opts.format, seq_n, selected_total, page_num, total_pages)?;

            if let Some(&page_id) = pages_map.get(&page_num) {
                let spec = PageOverlaySpec {
                    text: &text,
                    position: opts.position.to_overlay_position(),
                    font_size: opts.font_size,
                    rotation_deg: 0.0,
                    rgb: (r, g, b),
                    margin: opts.margin,
                };
                apply_overlay_to_page(&mut doc, page_id, font_id, gs_id, false, &spec)?;
            }
        }

        doc.compress();
        write_atomic(&out_path, opts.force, |file| {
            let mut writer = BufWriter::new(file);
            doc.save_to(&mut writer)
                .map_err(|e| std::io::Error::other(format!("Failed to write PDF: {e}")))?;
            writer.flush()?;
            Ok(())
        })?;
    }

    p.report(100, 100);

    Ok(NumberReport {
        input: src.to_path_buf(),
        output: out_path,
        format: opts.format.clone(),
        position: opts.position.as_str(),
        start: opts.start,
        font_size: opts.font_size,
        opacity: opts.opacity,
        color: canonical_color,
        total_pages,
        pages_numbered: target_pages,
        dry_run: opts.dry_run,
        was_encrypted,
    })
}
