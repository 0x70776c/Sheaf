//! PDF watermarking operations (`WatermarkPosition`, `WatermarkOptions`, `WatermarkReport`, `watermark`).

use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, StringFormat, dictionary};
use serde::{Deserialize, Serialize};

use crate::error::ensure_not_overwriting_inputs;
use crate::fs::{check_output_path, write_atomic};
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
}

/// Options for [`watermark`].
#[derive(Debug, Clone)]
pub struct WatermarkOptions {
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
    let trimmed = input.trim();
    let lower = trimmed.to_ascii_lowercase();
    let hex_str = match lower.as_str() {
        "gray" | "grey" => "#6B7280",
        "red" => "#DC2626",
        "blue" => "#2563EB",
        "black" => "#111827",
        "green" => "#16A34A",
        other => other,
    };

    let raw = hex_str.strip_prefix('#').unwrap_or(hex_str);
    if raw.len() != 6 || !raw.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(SheafError::InvalidSelection(format!(
            "Invalid watermark color '{trimmed}' (expected #RRGGBB hex or gray/red/blue/black/green)"
        )));
    }

    let r = u8::from_str_radix(&raw[0..2], 16).unwrap_or(107);
    let g = u8::from_str_radix(&raw[2..4], 16).unwrap_or(114);
    let b = u8::from_str_radix(&raw[4..6], 16).unwrap_or(128);

    Ok((
        (
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
        ),
        format!("#{}", raw.to_ascii_uppercase()),
    ))
}

/// Applies a customizable vector text watermark to all or selected pages of a PDF (`src`).
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
    let mut doc = Document::load_mem(&raw_bytes).map_err(|e| {
        let msg = e.to_string();
        if msg.to_ascii_lowercase().contains("encrypt") {
            SheafError::Encrypted
        } else {
            SheafError::InvalidPdf(format!("{}: {msg}", src.display()))
        }
    })?;

    if doc.is_encrypted() || doc.trailer.get(b"Encrypt").is_ok() {
        return Err(SheafError::Encrypted);
    }

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
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica-Bold",
            "Encoding" => "WinAnsiEncoding",
        });
        let gs_id = doc.add_object(dictionary! {
            "Type" => "ExtGState",
            "ca" => Object::Real(opts.opacity),
            "CA" => Object::Real(opts.opacity),
        });

        let total_steps = target_pages.len() as u64;
        for (idx, &page_num) in target_pages.iter().enumerate() {
            if p.is_cancelled() {
                return Err(SheafError::Cancelled);
            }
            p.report(idx as u64, total_steps);

            if let Some(&page_id) = pages_map.get(&page_num) {
                let (page_w, page_h) = get_page_dimensions(&doc, page_id);
                attach_watermark_resources(&mut doc, page_id, font_id, gs_id);
                let wm_ops = build_watermark_ops(
                    clean_text,
                    opts.position,
                    opts.font_size,
                    rotation_deg,
                    (r, g, b),
                    page_w,
                    page_h,
                );
                append_ops_to_page(&mut doc, page_id, wm_ops)?;
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

fn get_page_dimensions(doc: &Document, page_id: ObjectId) -> (f32, f32) {
    let mut current_id = Some(page_id);
    let mut depth = 0usize;
    while let Some(id) = current_id {
        depth += 1;
        if depth > 16 {
            break;
        }
        let Ok(Object::Dictionary(dict)) = doc.get_object(id) else {
            break;
        };
        if let Ok(mb) = dict.get(b"MediaBox") {
            let arr = match mb {
                Object::Array(a) => Some(a),
                Object::Reference(r) => doc.get_object(*r).ok().and_then(|o| o.as_array().ok()),
                _ => None,
            };
            if let Some(a) = arr
                && a.len() >= 4
                && let (Some(x0), Some(y0), Some(x1), Some(y1)) = (
                    obj_as_f32(&a[0]),
                    obj_as_f32(&a[1]),
                    obj_as_f32(&a[2]),
                    obj_as_f32(&a[3]),
                )
            {
                let w = (x1 - x0).abs();
                let h = (y1 - y0).abs();
                if w > 1.0 && h > 1.0 {
                    return (w, h);
                }
            }
        }
        current_id = dict.get(b"Parent").and_then(Object::as_reference).ok();
    }
    (612.0, 792.0)
}

fn obj_as_f32(obj: &Object) -> Option<f32> {
    match obj {
        Object::Real(v) => Some(*v),
        Object::Integer(v) => Some(*v as f32),
        _ => None,
    }
}

fn attach_watermark_resources(
    doc: &mut Document,
    page_id: ObjectId,
    font_id: ObjectId,
    gs_id: ObjectId,
) {
    // Ensure the page has its own Resources dictionary with /Font -> /SheafWmFont and /ExtGState -> /SheafWmGS
    let existing_res = doc
        .get_object(page_id)
        .ok()
        .and_then(|o| o.as_dict().ok())
        .and_then(|d| d.get(b"Resources").ok())
        .cloned();

    let mut res_dict = match existing_res {
        Some(Object::Dictionary(d)) => d,
        Some(Object::Reference(r)) => doc
            .get_object(r)
            .ok()
            .and_then(|o| o.as_dict().ok())
            .cloned()
            .unwrap_or_else(Dictionary::new),
        _ => Dictionary::new(),
    };

    // Font dictionary
    let mut font_dict = match res_dict.get(b"Font").ok().cloned() {
        Some(Object::Dictionary(d)) => d,
        Some(Object::Reference(r)) => doc
            .get_object(r)
            .ok()
            .and_then(|o| o.as_dict().ok())
            .cloned()
            .unwrap_or_else(Dictionary::new),
        _ => Dictionary::new(),
    };
    font_dict.set("SheafWmFont", Object::Reference(font_id));
    res_dict.set("Font", Object::Dictionary(font_dict));

    // ExtGState dictionary
    let mut gs_dict = match res_dict.get(b"ExtGState").ok().cloned() {
        Some(Object::Dictionary(d)) => d,
        Some(Object::Reference(r)) => doc
            .get_object(r)
            .ok()
            .and_then(|o| o.as_dict().ok())
            .cloned()
            .unwrap_or_else(Dictionary::new),
        _ => Dictionary::new(),
    };
    gs_dict.set("SheafWmGS", Object::Reference(gs_id));
    res_dict.set("ExtGState", Object::Dictionary(gs_dict));

    if let Ok(Object::Dictionary(page_dict)) = doc.get_object_mut(page_id) {
        page_dict.set("Resources", Object::Dictionary(res_dict));
    }
}

fn build_watermark_ops(
    text: &str,
    position: WatermarkPosition,
    font_size: f32,
    rotation_deg: f32,
    rgb: (f32, f32, f32),
    page_w: f32,
    page_h: f32,
) -> Vec<Operation> {
    let text_w = (text.chars().count() as f32) * font_size * 0.56;
    let text_h = font_size * 0.72;
    let margin = 36.0f32;

    let rad = rotation_deg.to_radians();
    let cos_a = rad.cos();
    let sin_a = rad.sin();

    // Compute anchor point (tx, ty) for the text baseline start
    let (tx, ty) = match position {
        WatermarkPosition::Diagonal | WatermarkPosition::Center => {
            let cx = page_w * 0.5;
            let cy = page_h * 0.5;
            let dx = -text_w * 0.5;
            let dy = -text_h * 0.5;
            (cx + dx * cos_a - dy * sin_a, cy + dx * sin_a + dy * cos_a)
        }
        WatermarkPosition::TopLeft => (margin, page_h - margin - text_h),
        WatermarkPosition::TopRight => (
            (page_w - margin - text_w).max(margin),
            page_h - margin - text_h,
        ),
        WatermarkPosition::BottomLeft => (margin, margin),
        WatermarkPosition::BottomRight => ((page_w - margin - text_w).max(margin), margin),
    };

    let win_ansi: Vec<u8> = text
        .chars()
        .map(|c| match c {
            '\u{0020}'..='\u{00FF}' => c as u8,
            _ => b'?',
        })
        .collect();

    vec![
        Operation::new("q", vec![]),
        Operation::new("gs", vec!["SheafWmGS".into()]),
        Operation::new(
            "rg",
            vec![
                Object::Real(rgb.0),
                Object::Real(rgb.1),
                Object::Real(rgb.2),
            ],
        ),
        Operation::new("BT", vec![]),
        Operation::new("Tf", vec!["SheafWmFont".into(), Object::Real(font_size)]),
        Operation::new(
            "Tm",
            vec![
                Object::Real(cos_a),
                Object::Real(sin_a),
                Object::Real(-sin_a),
                Object::Real(cos_a),
                Object::Real(tx),
                Object::Real(ty),
            ],
        ),
        Operation::new("Tj", vec![Object::String(win_ansi, StringFormat::Literal)]),
        Operation::new("ET", vec![]),
        Operation::new("Q", vec![]),
    ]
}

fn append_ops_to_page(
    doc: &mut Document,
    page_id: ObjectId,
    wm_ops: Vec<Operation>,
) -> Result<(), SheafError> {
    let encoded_wm = Content { operations: wm_ops }
        .encode()
        .map_err(|e| SheafError::InvalidPdf(format!("Failed to encode watermark stream: {e}")))?;

    let wm_stream_id = doc.add_object(Stream::new(dictionary! {}, encoded_wm));

    let existing_contents = doc
        .get_object(page_id)
        .ok()
        .and_then(|o| o.as_dict().ok())
        .and_then(|d| d.get(b"Contents").ok())
        .cloned();

    let new_contents = match existing_contents {
        Some(Object::Reference(id)) => {
            Object::Array(vec![Object::Reference(id), Object::Reference(wm_stream_id)])
        }
        Some(Object::Array(mut arr)) => {
            arr.push(Object::Reference(wm_stream_id));
            Object::Array(arr)
        }
        _ => Object::Reference(wm_stream_id),
    };

    if let Ok(Object::Dictionary(page_dict)) = doc.get_object_mut(page_id) {
        page_dict.set("Contents", new_contents);
    }

    Ok(())
}
