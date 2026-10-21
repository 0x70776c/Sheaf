//! Shared PDF vector overlay engine used by `watermark` and `number`.
//!
//! Handles:
//! - Inherited and indirect `/Resources` cloning without mutating parent `/Pages` nodes.
//! - Collision-free `/Font` (`SheafF*`) and `/ExtGState` (`SheafGS*`) resource naming.
//! - Encapsulating existing `/Contents` (single reference, array, or empty) in `q … Q`.
//! - Page geometry resolution (`CropBox` / `MediaBox` offsets and `/Rotate` `0`/`90`/`180`/`270`).
//! - Strict `WinAnsiEncoding` validation returning [`SheafError::Unsupported`] with a hint on non-WinAnsi characters.

use std::path::Path;

use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, StringFormat, dictionary};
use serde::{Deserialize, Serialize};

use crate::SheafError;
use crate::ops::pages::get_effective_page_rotation;

/// Placement preset for a vector text overlay on a PDF page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverlayPosition {
    /// Centered on the page and rotated 45° counter-clockwise.
    #[default]
    Diagonal,
    /// Centered horizontally and vertically (`0°` default rotation).
    Center,
    /// Top-left corner inset by margin.
    TopLeft,
    /// Top-center inset by margin.
    TopCenter,
    /// Top-right corner inset by margin.
    TopRight,
    /// Bottom-left corner inset by margin.
    BottomLeft,
    /// Bottom-center inset by margin.
    BottomCenter,
    /// Bottom-right corner inset by margin.
    BottomRight,
}

impl OverlayPosition {
    /// Parses a position identifier (`"diagonal"`, `"center"`, `"top-left"`, `"top-center"`,
    /// `"top-right"`, `"bottom-left"`, `"bottom-center"`, `"bottom-right"`).
    ///
    /// # Examples
    ///
    /// ```
    /// use sheaf_core::ops::overlay::OverlayPosition;
    ///
    /// assert_eq!(OverlayPosition::parse("diagonal").unwrap(), OverlayPosition::Diagonal);
    /// assert_eq!(OverlayPosition::parse("bottom-center").unwrap(), OverlayPosition::BottomCenter);
    /// assert!(OverlayPosition::parse("invalid").is_err());
    /// ```
    pub fn parse(input: &str) -> Result<Self, SheafError> {
        match input.trim().to_ascii_lowercase().as_str() {
            "diagonal" => Ok(Self::Diagonal),
            "center" => Ok(Self::Center),
            "top-left" => Ok(Self::TopLeft),
            "top-center" => Ok(Self::TopCenter),
            "top-right" => Ok(Self::TopRight),
            "bottom-left" => Ok(Self::BottomLeft),
            "bottom-center" => Ok(Self::BottomCenter),
            "bottom-right" => Ok(Self::BottomRight),
            other => Err(SheafError::InvalidSelection(format!(
                "Invalid overlay position '{other}' (expected: diagonal, center, top-left, top-center, top-right, bottom-left, bottom-center, or bottom-right)"
            ))),
        }
    }

    /// Stable kebab-case identifier.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Diagonal => "diagonal",
            Self::Center => "center",
            Self::TopLeft => "top-left",
            Self::TopCenter => "top-center",
            Self::TopRight => "top-right",
            Self::BottomLeft => "bottom-left",
            Self::BottomCenter => "bottom-center",
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

/// Effective visible page geometry in PDF user space, including `/Rotate` and box origin offset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageBox {
    /// Left X origin of the effective box (`CropBox` or `MediaBox`).
    pub x0: f32,
    /// Bottom Y origin of the effective box (`CropBox` or `MediaBox`).
    pub y0: f32,
    /// Unrotated box width in PDF points.
    pub width: f32,
    /// Unrotated box height in PDF points.
    pub height: f32,
    /// Effective clockwise page rotation (`0`, `90`, `180`, or `270`).
    pub rotate: i32,
}

impl PageBox {
    /// Returns the visual `(width, height)` after applying the page's `/Rotate` attribute.
    ///
    /// # Examples
    ///
    /// ```
    /// use sheaf_core::ops::overlay::PageBox;
    ///
    /// let b = PageBox { x0: 0.0, y0: 0.0, width: 612.0, height: 792.0, rotate: 90 };
    /// assert_eq!(b.visual_dimensions(), (792.0, 612.0));
    /// ```
    pub fn visual_dimensions(&self) -> (f32, f32) {
        if self.rotate == 90 || self.rotate == 270 {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        }
    }

    /// Returns the 6-element PDF `cm` transformation matrix `[a, b, c, d, e, f]` that maps
    /// visual page coordinates `(0..W_vis, 0..H_vis)` into unrotated PDF user space, compensating
    /// for both `/Rotate` (`0`, `90`, `180`, `270`) and box origin offset `(x0, y0)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sheaf_core::ops::overlay::PageBox;
    ///
    /// let b = PageBox { x0: 10.0, y0: 20.0, width: 612.0, height: 792.0, rotate: 0 };
    /// assert_eq!(b.visual_to_user_ctm(), [1.0, 0.0, 0.0, 1.0, 10.0, 20.0]);
    /// ```
    pub fn visual_to_user_ctm(&self) -> [f32; 6] {
        let (x0, y0, w, h) = (self.x0, self.y0, self.width, self.height);
        match self.rotate {
            90 => [0.0, 1.0, -1.0, 0.0, x0 + w, y0],
            180 => [-1.0, 0.0, 0.0, -1.0, x0 + w, y0 + h],
            270 => [0.0, -1.0, 1.0, 0.0, x0, y0 + h],
            _ => [1.0, 0.0, 0.0, 1.0, x0, y0],
        }
    }
}

/// Specification for drawing a single text overlay on a page.
#[derive(Debug, Clone)]
pub struct PageOverlaySpec<'a> {
    /// Text to render (must be valid `WinAnsiEncoding`).
    pub text: &'a str,
    /// Placement preset on the visual page.
    pub position: OverlayPosition,
    /// Font size in PDF points (`6.0..=300.0`).
    pub font_size: f32,
    /// Additional text rotation in degrees counter-clockwise relative to visual page.
    pub rotation_deg: f32,
    /// Normalized RGB color components in `0.0..=1.0`.
    pub rgb: (f32, f32, f32),
    /// Margin inset from page edges in PDF points (used for corner/edge positions).
    pub margin: f32,
}

/// Encodes a Unicode string into PDF `WinAnsiEncoding` (Windows-1252) bytes.
///
/// Returns [`SheafError::Unsupported`] with an actionable hint if any character cannot be
/// represented in `WinAnsiEncoding`.
///
/// # Examples
///
/// ```
/// use sheaf_core::ops::overlay::encode_win_ansi;
///
/// assert_eq!(encode_win_ansi("Page 1 - €").unwrap(), vec![b'P', b'a', b'g', b'e', b' ', b'1', b' ', b'-', b' ', 0x80]);
/// assert!(encode_win_ansi("Confidential 🔒").is_err());
/// ```
pub fn encode_win_ansi(text: &str) -> Result<Vec<u8>, SheafError> {
    let mut out = Vec::with_capacity(text.len());
    for ch in text.chars() {
        let byte = match ch {
            '\u{0020}'..='\u{007E}' | '\u{00A0}'..='\u{00FF}' => ch as u8,
            '\u{20AC}' => 0x80,
            '\u{201A}' => 0x82,
            '\u{0192}' => 0x83,
            '\u{201E}' => 0x84,
            '\u{2026}' => 0x85,
            '\u{2020}' => 0x86,
            '\u{2021}' => 0x87,
            '\u{02C6}' => 0x88,
            '\u{2030}' => 0x89,
            '\u{0160}' => 0x8A,
            '\u{2039}' => 0x8B,
            '\u{0152}' => 0x8C,
            '\u{017D}' => 0x8E,
            '\u{2018}' => 0x91,
            '\u{2019}' => 0x92,
            '\u{201C}' => 0x93,
            '\u{201D}' => 0x94,
            '\u{2022}' => 0x95,
            '\u{2013}' => 0x96,
            '\u{2014}' => 0x97,
            '\u{02DC}' => 0x98,
            '\u{2122}' => 0x99,
            '\u{0161}' => 0x9A,
            '\u{203A}' => 0x9B,
            '\u{0153}' => 0x9C,
            '\u{017E}' => 0x9E,
            '\u{0178}' => 0x9F,
            _ => {
                return Err(SheafError::Unsupported(format!(
                    "Character '{}' (U+{:04X}) is not supported by PDF WinAnsiEncoding. Hint: Use standard Latin/Western characters supported by built-in Type1 fonts.",
                    ch, ch as u32
                )));
            }
        };
        out.push(byte);
    }
    Ok(out)
}

/// Parses a `#RRGGBB` hex string or named color (`gray`, `red`, `blue`, `black`, `green`) into
/// normalized RGB `(r, g, b)` in `0.0..=1.0` and a canonical `#RRGGBB` string.
///
/// # Examples
///
/// ```
/// use sheaf_core::ops::overlay::parse_overlay_color;
///
/// let (rgb, hex) = parse_overlay_color("red").unwrap();
/// assert_eq!(hex, "#DC2626");
/// assert!(rgb.0 > 0.8);
/// ```
pub fn parse_overlay_color(input: &str) -> Result<((f32, f32, f32), String), SheafError> {
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
            "Invalid color '{trimmed}' (expected #RRGGBB hex or gray/red/blue/black/green)"
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

/// Resolves the effective visible [`PageBox`] (`CropBox` with fallback to `MediaBox`, plus `/Rotate`)
/// for `page_id`, walking up `/Parent` nodes when attributes are inherited.
pub fn get_effective_page_box(doc: &Document, page_id: ObjectId) -> PageBox {
    let crop_box = find_inherited_box(doc, page_id, b"CropBox");
    let media_box =
        find_inherited_box(doc, page_id, b"MediaBox").unwrap_or((0.0, 0.0, 612.0, 792.0));
    let (x0, y0, width, height) = crop_box.unwrap_or(media_box);
    let rotate = get_effective_page_rotation(doc, page_id);
    PageBox {
        x0,
        y0,
        width,
        height,
        rotate,
    }
}

fn find_inherited_box(
    doc: &Document,
    page_id: ObjectId,
    key: &[u8],
) -> Option<(f32, f32, f32, f32)> {
    let mut current_id = Some(page_id);
    let mut depth = 0usize;
    while let Some(id) = current_id {
        depth += 1;
        if depth > 32 {
            break;
        }
        let Ok(dict) = doc.get_dictionary(id) else {
            break;
        };
        if let Ok(box_obj) = dict.get_deref(key, doc)
            && let Ok(arr) = box_obj.as_array()
            && arr.len() >= 4
            && let (Some(x0), Some(y0), Some(x1), Some(y1)) = (
                obj_as_f32(&arr[0]),
                obj_as_f32(&arr[1]),
                obj_as_f32(&arr[2]),
                obj_as_f32(&arr[3]),
            )
        {
            let w = (x1 - x0).abs();
            let h = (y1 - y0).abs();
            if w > 1.0 && h > 1.0 {
                return Some((x0.min(x1), y0.min(y1), w, h));
            }
        }
        current_id = dict.get(b"Parent").ok().and_then(|p| p.as_reference().ok());
    }
    None
}

fn obj_as_f32(obj: &Object) -> Option<f32> {
    match obj {
        Object::Real(v) => Some(*v),
        Object::Integer(v) => Some(*v as f32),
        _ => None,
    }
}

/// Creates a Type1 Helvetica font object (`Helvetica` or `Helvetica-Bold`) and an `ExtGState`
/// opacity object in `doc`.
pub fn create_overlay_shared_objects(
    doc: &mut Document,
    base_font: &str,
    opacity: f32,
) -> (ObjectId, ObjectId) {
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => base_font,
        "Encoding" => "WinAnsiEncoding",
    });
    let gs_id = doc.add_object(dictionary! {
        "Type" => "ExtGState",
        "ca" => Object::Real(opacity),
        "CA" => Object::Real(opacity),
    });
    (font_id, gs_id)
}

/// Resolves direct or inherited `/Resources` for `page_id`, copies them into `page_id`'s own
/// dictionary (leaving `/Parent` and shared indirect `/Resources` untouched), and registers
/// `font_id` and `gs_id` under collision-free `SheafF*` and `SheafGS*` names.
pub fn attach_overlay_resources(
    doc: &mut Document,
    page_id: ObjectId,
    font_id: ObjectId,
    gs_id: ObjectId,
) -> (String, String) {
    let mut res_dict = resolve_inherited_resources_copy(doc, page_id);

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

    let font_key = next_collision_free_name(&font_dict, "SheafF");
    font_dict.set(font_key.as_bytes(), Object::Reference(font_id));
    res_dict.set("Font", Object::Dictionary(font_dict));

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

    let gs_key = next_collision_free_name(&gs_dict, "SheafGS");
    gs_dict.set(gs_key.as_bytes(), Object::Reference(gs_id));
    res_dict.set("ExtGState", Object::Dictionary(gs_dict));

    if let Ok(Object::Dictionary(page_dict)) = doc.get_object_mut(page_id) {
        page_dict.set("Resources", Object::Dictionary(res_dict));
    }

    (font_key, gs_key)
}

fn resolve_inherited_resources_copy(doc: &Document, page_id: ObjectId) -> Dictionary {
    let mut current_id = Some(page_id);
    let mut depth = 0usize;
    while let Some(id) = current_id {
        depth += 1;
        if depth > 32 {
            break;
        }
        let Ok(dict) = doc.get_dictionary(id) else {
            break;
        };
        if let Ok(res_obj) = dict.get(b"Resources") {
            match res_obj {
                Object::Dictionary(d) => return d.clone(),
                Object::Reference(r) => {
                    if let Ok(d) = doc.get_dictionary(*r) {
                        return d.clone();
                    }
                }
                _ => {}
            }
        }
        current_id = dict.get(b"Parent").ok().and_then(|p| p.as_reference().ok());
    }
    Dictionary::new()
}

fn next_collision_free_name(dict: &Dictionary, prefix: &str) -> String {
    let mut idx = 0u32;
    loop {
        let candidate = format!("{prefix}{idx}");
        if !dict.has(candidate.as_bytes()) {
            return candidate;
        }
        idx += 1;
    }
}

/// Estimates text width in PDF points for Helvetica / Helvetica-Bold at `font_size`.
pub fn estimate_helvetica_text_width(text: &str, font_size: f32, bold: bool) -> f32 {
    let total_units: u32 = text
        .chars()
        .map(|c| match c {
            ' ' => 278,
            'i' | 'l' | 'I' | '!' | '|' | '.' | ',' | ':' | ';' | '\'' => {
                if bold {
                    333
                } else {
                    278
                }
            }
            'm' | 'w' | 'M' | 'W' => {
                if bold {
                    889
                } else {
                    833
                }
            }
            'A'..='Z' => {
                if bold {
                    722
                } else {
                    667
                }
            }
            '0'..='9' => 556,
            '-' | '(' | ')' | '[' | ']' | '/' => 333,
            _ => {
                if bold {
                    580
                } else {
                    520
                }
            }
        })
        .sum();
    (total_units as f32 / 1000.0) * font_size
}

/// Applies a single vector text overlay to `page_id` in `doc`.
///
/// - Clones direct/inherited `/Resources` onto `page_id` and injects collision-free `SheafF*`
///   and `SheafGS*` entries.
/// - Encapsulates existing `/Contents` (if any) in `q … Q` before drawing the overlay stream.
/// - Compensates for `CropBox`/`MediaBox` origin offsets and `/Rotate` (`0`, `90`, `180`, `270`).
pub fn apply_overlay_to_page(
    doc: &mut Document,
    page_id: ObjectId,
    font_id: ObjectId,
    gs_id: ObjectId,
    bold: bool,
    spec: &PageOverlaySpec<'_>,
) -> Result<(), SheafError> {
    let win_ansi_bytes = encode_win_ansi(spec.text)?;
    let page_box = get_effective_page_box(doc, page_id);
    let (font_key, gs_key) = attach_overlay_resources(doc, page_id, font_id, gs_id);

    let overlay_ops = build_overlay_operations(
        &win_ansi_bytes,
        spec.text,
        bold,
        &font_key,
        &gs_key,
        &page_box,
        spec,
    );

    wrap_and_append_page_contents(doc, page_id, overlay_ops)
}

fn build_overlay_operations(
    win_ansi_bytes: &[u8],
    raw_text: &str,
    bold: bool,
    font_key: &str,
    gs_key: &str,
    page_box: &PageBox,
    spec: &PageOverlaySpec<'_>,
) -> Vec<Operation> {
    let (vis_w, vis_h) = page_box.visual_dimensions();
    let text_w = estimate_helvetica_text_width(raw_text, spec.font_size, bold);
    let text_h = spec.font_size * 0.72;
    let margin = spec.margin;

    let rad = spec.rotation_deg.to_radians();
    let cos_a = rad.cos();
    let sin_a = rad.sin();

    let (tx, ty) = match spec.position {
        OverlayPosition::Diagonal | OverlayPosition::Center => {
            let cx = vis_w * 0.5;
            let cy = vis_h * 0.5;
            let dx = -text_w * 0.5;
            let dy = -text_h * 0.5;
            (cx + dx * cos_a - dy * sin_a, cy + dx * sin_a + dy * cos_a)
        }
        OverlayPosition::TopLeft => (margin, (vis_h - margin - text_h).max(margin)),
        OverlayPosition::TopCenter => (
            ((vis_w - text_w) * 0.5).max(margin),
            (vis_h - margin - text_h).max(margin),
        ),
        OverlayPosition::TopRight => (
            (vis_w - margin - text_w).max(margin),
            (vis_h - margin - text_h).max(margin),
        ),
        OverlayPosition::BottomLeft => (margin, margin),
        OverlayPosition::BottomCenter => (((vis_w - text_w) * 0.5).max(margin), margin),
        OverlayPosition::BottomRight => ((vis_w - margin - text_w).max(margin), margin),
    };

    let ctm = page_box.visual_to_user_ctm();

    vec![
        Operation::new("q", vec![]),
        Operation::new(
            "cm",
            vec![
                Object::Real(ctm[0]),
                Object::Real(ctm[1]),
                Object::Real(ctm[2]),
                Object::Real(ctm[3]),
                Object::Real(ctm[4]),
                Object::Real(ctm[5]),
            ],
        ),
        Operation::new("gs", vec![gs_key.into()]),
        Operation::new(
            "rg",
            vec![
                Object::Real(spec.rgb.0),
                Object::Real(spec.rgb.1),
                Object::Real(spec.rgb.2),
            ],
        ),
        Operation::new("BT", vec![]),
        Operation::new("Tf", vec![font_key.into(), Object::Real(spec.font_size)]),
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
        Operation::new(
            "Tj",
            vec![Object::String(
                win_ansi_bytes.to_vec(),
                StringFormat::Literal,
            )],
        ),
        Operation::new("ET", vec![]),
        Operation::new("Q", vec![]),
    ]
}

fn wrap_and_append_page_contents(
    doc: &mut Document,
    page_id: ObjectId,
    mut overlay_ops: Vec<Operation>,
) -> Result<(), SheafError> {
    let existing_contents = doc
        .get_object(page_id)
        .ok()
        .and_then(|o| o.as_dict().ok())
        .and_then(|d| d.get(b"Contents").ok())
        .cloned();

    let mut existing_refs: Vec<Object> = Vec::new();
    match existing_contents {
        Some(Object::Reference(id)) => {
            existing_refs.push(Object::Reference(id));
        }
        Some(Object::Array(arr)) => {
            for item in arr {
                match item {
                    Object::Reference(id) => existing_refs.push(Object::Reference(id)),
                    Object::Stream(s) => {
                        let id = doc.add_object(Object::Stream(s));
                        existing_refs.push(Object::Reference(id));
                    }
                    _ => {}
                }
            }
        }
        Some(Object::Stream(s)) => {
            let id = doc.add_object(Object::Stream(s));
            existing_refs.push(Object::Reference(id));
        }
        _ => {}
    }

    let new_contents = if existing_refs.is_empty() {
        let encoded_overlay = Content {
            operations: overlay_ops,
        }
        .encode()
        .map_err(|e| SheafError::InvalidPdf(format!("Failed to encode overlay stream: {e}")))?;
        let overlay_stream_id = doc.add_object(Stream::new(dictionary! {}, encoded_overlay));
        Object::Reference(overlay_stream_id)
    } else {
        // Wrap original page contents in `q ... Q` so any unclosed CTM or graphics state changes
        // in the original stream do not distort the overlay.
        let q_bytes = Content {
            operations: vec![Operation::new("q", vec![])],
        }
        .encode()
        .map_err(|e| SheafError::InvalidPdf(format!("Failed to encode q prefix stream: {e}")))?;
        let q_stream_id = doc.add_object(Stream::new(dictionary! {}, q_bytes));

        let mut suffix_ops = Vec::with_capacity(overlay_ops.len() + 1);
        suffix_ops.push(Operation::new("Q", vec![]));
        suffix_ops.append(&mut overlay_ops);

        let suffix_bytes = Content {
            operations: suffix_ops,
        }
        .encode()
        .map_err(|e| SheafError::InvalidPdf(format!("Failed to encode overlay stream: {e}")))?;
        let suffix_stream_id = doc.add_object(Stream::new(dictionary! {}, suffix_bytes));

        let mut arr = Vec::with_capacity(existing_refs.len() + 2);
        arr.push(Object::Reference(q_stream_id));
        arr.extend(existing_refs);
        arr.push(Object::Reference(suffix_stream_id));
        Object::Array(arr)
    };

    if let Ok(Object::Dictionary(page_dict)) = doc.get_object_mut(page_id) {
        page_dict.set("Contents", new_contents);
    }

    Ok(())
}

/// Applies an overlay to a single 1-based `page` of `src` strictly in memory and renders it to PNG bytes
/// at `width_px` width for live UI previews.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use sheaf_core::ops::overlay::{OverlayPosition, preview_overlay_page};
///
/// let png = preview_overlay_page(
///     Path::new("input.pdf"),
///     1,
///     "DRAFT",
///     OverlayPosition::Diagonal,
///     48.0,
///     0.25,
///     45.0,
///     "#6B7280",
///     36.0,
///     true,
///     240,
/// ).unwrap();
/// assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
/// ```
#[allow(clippy::too_many_arguments)]
pub fn preview_overlay_page(
    src: &Path,
    page: u32,
    text: &str,
    position: OverlayPosition,
    font_size: f32,
    opacity: f32,
    rotation_deg: f32,
    color: &str,
    margin: f32,
    bold: bool,
    width_px: u32,
) -> Result<Vec<u8>, SheafError> {
    let clean_text = text.trim();
    if clean_text.is_empty() {
        return crate::ops::render::render_page(src, page, width_px);
    }
    let _ = encode_win_ansi(clean_text)?;
    let (rgb, _) = parse_overlay_color(color)?;

    let raw_bytes = std::fs::read(src)?;
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
    let total = pages_map.len() as u32;
    let &page_id = pages_map
        .get(&page)
        .ok_or(SheafError::PageOutOfRange { page, total })?;

    let base_font = if bold { "Helvetica-Bold" } else { "Helvetica" };
    let (font_id, gs_id) = create_overlay_shared_objects(&mut doc, base_font, opacity);
    let spec = PageOverlaySpec {
        text: clean_text,
        position,
        font_size: font_size.clamp(6.0, 300.0),
        rotation_deg,
        rgb,
        margin: margin.clamp(0.0, 200.0),
    };
    apply_overlay_to_page(&mut doc, page_id, font_id, gs_id, bold, &spec)?;

    let mut pdf_buf = Vec::new();
    doc.save_to(&mut pdf_buf)
        .map_err(|e| SheafError::InvalidPdf(format!("Failed to serialize preview PDF: {e}")))?;

    crate::ops::render::render_pdf_bytes_page(&pdf_buf, page, width_px)
}
