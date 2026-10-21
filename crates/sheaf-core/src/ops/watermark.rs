//! PDF watermarking operations (`WatermarkPosition`, `WatermarkOptions`, `WatermarkReport`, `watermark`).

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

/// Placement preset for a PDF text watermark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WatermarkPosition {
    /// Centered on the page and rotated 45° counter-clockwise (default).
    #[default]
    Diagonal,
    /// Centered horizontally and vertically (`0°` default rotation).
    Center,
    /// Top-left corner inset by 36 pt.
    TopLeft,
    /// Top-right corner inset by 36 pt.
    TopRight,
    /// Bottom-left corner inset by 36 pt.
    BottomLeft,
    /// Bottom-right corner inset by 36 pt.
    BottomRight,
}

impl WatermarkPosition {
    /// Parses a position identifier (`"diagonal"`, `"center"`, `"top-left"`, `"top-right"`, `"bottom-left"`, `"bottom-right"`).
    ///
    /// # Examples
    ///
    /// ```
    /// use sheaf_core::WatermarkPosition;
    ///
    /// assert_eq!(WatermarkPosition::parse("diagonal").unwrap(), WatermarkPosition::Diagonal);
    /// assert_eq!(WatermarkPosition::parse("bottom-right").unwrap(), WatermarkPosition::BottomRight);
    /// assert!(WatermarkPosition::parse("middle").is_err());
    /// ```
    pub fn parse(input: &str) -> Result<Self, SheafError> {
        match input.trim().to_ascii_lowercase().as_str() {
            "diagonal" => Ok(Self::Diagonal),
            "center" => Ok(Self::Center),
            "top-left" => Ok(Self::TopLeft),
            "top-right" => Ok(Self::TopRight),
            "bottom-left" => Ok(Self::BottomLeft),
            "bottom-right" => Ok(Self::BottomRight),
            other => Err(SheafError::InvalidSelection(format!(
                "Invalid watermark position '{other}' (expected: diagonal, center, top-left, top-right, bottom-left, or bottom-right)"
            ))),
        }
    }

    /// Stable kebab-case identifier.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Diagonal => "diagonal",
            Self::Center => "center",
            Self::TopLeft => "top-left",
            Self::TopRight => "top-right",
            Self::BottomLeft => "bottom-left",
            Self::BottomRight => "bottom-right",
        }
    }

    /// Default rotation angle in degrees (`45.0` for `Diagonal`, `0.0` otherwise).
    pub fn default_rotation_deg(&self) -> f32 {
        match self {
            Self::Diagonal => 45.0,
            _ => 0.0,
        }
    }

    /// Converts this watermark position into the shared [`OverlayPosition`].
    pub fn to_overlay_position(self) -> OverlayPosition {
        match self {
            Self::Diagonal => OverlayPosition::Diagonal,
            Self::Center => OverlayPosition::Center,
            Self::TopLeft => OverlayPosition::TopLeft,
            Self::TopRight => OverlayPosition::TopRight,
            Self::BottomLeft => OverlayPosition::BottomLeft,
            Self::BottomRight => OverlayPosition::BottomRight,
        }
    }
}

/// Options for [`watermark`].
#[derive(Debug, Clone)]
pub struct WatermarkOptions {
    /// Loading and decryption options for input PDF.
    pub load: LoadOptions,
    /// Watermark text to stamp onto target pages.
    pub text: String,
    /// Placement on the page.
    pub position: WatermarkPosition,
    /// Font size in PDF points (`6.0..=300.0`, default `48.0`).
    pub font_size: f32,
    /// Opacity (`0.05..=1.0`, default `0.25`).
    pub opacity: f32,
    /// Optional explicit rotation angle in degrees (overrides position default).
    pub rotation_deg: Option<f32>,
    /// Color as `#RRGGBB` hex or named preset (`gray`, `red`, `blue`, `black`; default `#6B7280`).
    pub color: String,
    /// Optional 1-based page selection (`None` watermarks all pages).
    pub pages: Option<String>,
    /// Target output PDF path (defaults to `{stem}_watermarked.pdf`).
    pub out: Option<PathBuf>,
    /// Overwrite existing output file if `true`.
    pub force: bool,
    /// Compute watermark plan without writing the output file if `true`.
    pub dry_run: bool,
}

impl Default for WatermarkOptions {
    fn default() -> Self {
        Self {
            load: LoadOptions::default(),
            text: "CONFIDENTIAL".to_string(),
            position: WatermarkPosition::Diagonal,
            font_size: 48.0,
            opacity: 0.25,
            rotation_deg: None,
            color: "#6B7280".to_string(),
            pages: None,
            out: None,
            force: false,
            dry_run: false,
        }
    }
}

/// Report returned by [`watermark`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WatermarkReport {
    /// Input PDF file path.
    pub input: PathBuf,
    /// Output PDF file path.
    pub output: PathBuf,
    /// Watermark text applied.
    pub text: String,
    /// Position preset name.
    pub position: &'static str,
    /// Font size in PDF points.
    pub font_size: f32,
    /// Opacity (`0.05..=1.0`).
    pub opacity: f32,
    /// Applied rotation angle in degrees.
    pub rotation_deg: f32,
    /// Normalized `#RRGGBB` color string.
    pub color: String,
    /// Total pages in the document.
    pub total_pages: u32,
    /// 1-based page numbers where the watermark was applied.
    pub pages_watermarked: Vec<u32>,
    /// Indicates whether `--dry-run` was active.
    pub dry_run: bool,
}

/// Parses a `#RRGGBB` hex string or named color (`gray`, `red`, `blue`, `black`) into normalized RGB `(r, g, b)` in `0.0..=1.0` and a canonical `#RRGGBB` string.
///
/// # Examples
///
/// ```
/// use sheaf_core::ops::watermark::parse_watermark_color;
///
/// let (rgb, hex) = parse_watermark_color("red").unwrap();
/// assert_eq!(hex, "#DC2626");
/// assert!(rgb.0 > 0.8);
/// ```
pub fn parse_watermark_color(input: &str) -> Result<((f32, f32, f32), String), SheafError> {
    parse_overlay_color(input)
}

/// Applies a customizable vector text watermark to all or selected pages of a PDF (`src`)
/// using the shared [`crate::ops::overlay`] engine.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use sheaf_core::{NoProgress, WatermarkOptions, watermark};
///
/// let opts = WatermarkOptions {
///     text: "DRAFT".to_string(),
///     ..WatermarkOptions::default()
/// };
/// let report = watermark(Path::new("input.pdf"), &opts, &NoProgress).unwrap();
/// println!("Watermarked {} pages", report.pages_watermarked.len());
/// ```
pub fn watermark(
    src: &Path,
    opts: &WatermarkOptions,
    p: &dyn Progress,
) -> Result<WatermarkReport, SheafError> {
    let clean_text = opts.text.trim();
    if clean_text.is_empty() {
        return Err(SheafError::InvalidSelection(
            "Watermark text cannot be empty".to_string(),
        ));
    }
    // Validate WinAnsiEncoding up front so unsupported characters fail deterministically with a hint.
    let _ = encode_win_ansi(clean_text)?;

    if !(6.0..=300.0).contains(&opts.font_size) {
        return Err(SheafError::InvalidSelection(format!(
            "Watermark font size must be between 6 and 300 pt (got {})",
            opts.font_size
        )));
    }
    if !(0.05..=1.0).contains(&opts.opacity) {
        return Err(SheafError::InvalidSelection(format!(
            "Watermark opacity must be between 0.05 and 1.0 (got {})",
            opts.opacity
        )));
    }
    let ((r, g, b), canonical_color) = parse_watermark_color(&opts.color)?;

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
            parent.join(format!("{stem}_watermarked.pdf"))
        }
    };
    ensure_not_overwriting_inputs(&[src.to_path_buf()], &out_path)?;
    if !opts.dry_run {
        check_output_path(&out_path, opts.force)?;
    }

    let raw_bytes = fs::read(src)?;
    let loaded = load_pdf_from_bytes(&raw_bytes, src, &opts.load)?;
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

    let rotation_deg = opts
        .rotation_deg
        .unwrap_or_else(|| opts.position.default_rotation_deg());

    if !opts.dry_run {
        let (font_id, gs_id) =
            create_overlay_shared_objects(&mut doc, "Helvetica-Bold", opts.opacity);
        let spec = PageOverlaySpec {
            text: clean_text,
            position: opts.position.to_overlay_position(),
            font_size: opts.font_size,
            rotation_deg,
            rgb: (r, g, b),
            margin: 36.0,
        };

        let total_steps = target_pages.len() as u64;
        for (idx, &page_num) in target_pages.iter().enumerate() {
            if p.is_cancelled() {
                return Err(SheafError::Cancelled);
            }
            p.report(idx as u64, total_steps);

            if let Some(&page_id) = pages_map.get(&page_num) {
                apply_overlay_to_page(&mut doc, page_id, font_id, gs_id, true, &spec)?;
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

    Ok(WatermarkReport {
        input: src.to_path_buf(),
        output: out_path,
        text: clean_text.to_string(),
        position: opts.position.as_str(),
        font_size: opts.font_size,
        opacity: opts.opacity,
        rotation_deg,
        color: canonical_color,
        total_pages,
        pages_watermarked: target_pages,
        dry_run: opts.dry_run,
    })
}
