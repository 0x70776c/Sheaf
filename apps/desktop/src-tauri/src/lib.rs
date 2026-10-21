//! Tauri 2 desktop application library for **Sheaf**.
//!
//! Acts as a thin IPC bridge between the Svelte 5 frontend and `sheaf-core`.

#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde::Serialize;
use sheaf_core::{
    CapabilitiesReport, CompressOptions, CompressPreset, CompressReport, ConvertPageSize,
    ConvertReport, DEFAULT_THUMBNAIL_WIDTH_PX, ImageFormat, ImagesToPdfOptions, InfoReport,
    MarkdownToPdfOptions, MergeOptions, MergeReport, NoProgress, NumberOptions, NumberPosition,
    NumberReport, OverlayPosition, PageOpOptions, PageSelection, PagesReport, PdfToImagesOptions,
    PdfToMarkdownOptions, Progress, ProtectOptions, ProtectReport, SheafError, SplitMode,
    SplitOptions, SplitReport, UnlockOptions, WatermarkOptions, WatermarkPosition, WatermarkReport,
    capabilities, compress_with_options, delete_pages, extract_pages, format_page_number,
    images_to_pdf, info, markdown_to_pdf, merge_with_options, number, pdf_to_images,
    pdf_to_markdown, preview_overlay_page, protect, render_thumbnails, reorder_pages, rotate_pages,
    split, unlock, watermark,
};
use tauri::{AppHandle, Emitter, State};

/// Shared application state managed by Tauri.
#[derive(Default)]
pub struct AppState {
    /// Atomic cancellation flag for the currently running operation.
    pub cancelled: Arc<AtomicBool>,
}

/// Structured error returned by Tauri commands to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct CommandError {
    /// Stable machine-readable error code from [`SheafError::code`].
    pub code: &'static str,
    /// Human-readable error message.
    pub message: String,
    /// Optional recovery hint.
    pub hint: Option<String>,
}

impl From<SheafError> for CommandError {
    fn from(err: SheafError) -> Self {
        Self {
            code: err.code(),
            message: err.to_string(),
            hint: err.hint(),
        }
    }
}

/// Rendered thumbnail item returned to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct ThumbnailItem {
    /// 1-based page number.
    pub page: u32,
    /// Rendered PNG width in pixels.
    pub width_px: u32,
    /// Rendered PNG height in pixels.
    pub height_px: u32,
    /// `data:image/png;base64,...` URL ready for `<img>` rendering.
    pub data_url: String,
}

/// Progress event payload emitted on `sheaf://progress`.
#[derive(Debug, Clone, Serialize)]
pub struct ProgressEventPayload {
    /// Operation identifier (`"thumbnails"`, `"merge"`, `"split"`, `"pages"`).
    pub op: &'static str,
    /// Completed work units.
    pub done: u64,
    /// Total work units.
    pub total: u64,
}

struct TauriProgress {
    app: AppHandle,
    op: &'static str,
    cancelled: Arc<AtomicBool>,
}

impl Progress for TauriProgress {
    fn report(&self, done: u64, total: u64) {
        let _ = self.app.emit(
            "sheaf://progress",
            ProgressEventPayload {
                op: self.op,
                done,
                total,
            },
        );
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

fn parse_png_dimensions(png_bytes: &[u8]) -> (u32, u32) {
    if png_bytes.len() >= 24
        && &png_bytes[0..8] == b"\x89PNG\r\n\x1a\n"
        && &png_bytes[12..16] == b"IHDR"
    {
        let width =
            u32::from_be_bytes([png_bytes[16], png_bytes[17], png_bytes[18], png_bytes[19]]);
        let height =
            u32::from_be_bytes([png_bytes[20], png_bytes[21], png_bytes[22], png_bytes[23]]);
        (width, height)
    } else {
        (DEFAULT_THUMBNAIL_WIDTH_PX, DEFAULT_THUMBNAIL_WIDTH_PX)
    }
}

fn default_merged_path(first_input: &Path) -> PathBuf {
    let parent = first_input
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let stem = first_input
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("document");
    parent.join(format!("{stem}_merged.pdf"))
}

fn parse_selection_for_file(input: &Path, spec: &str) -> Result<PageSelection, SheafError> {
    let info_report = info(&[input.to_path_buf()], &NoProgress)?;
    let file_info = info_report
        .files
        .first()
        .ok_or(SheafError::EmptySelection)?;
    if file_info.encrypted {
        return Err(SheafError::Encrypted);
    }
    let total_pages = file_info.pages.unwrap_or(0);
    PageSelection::parse(spec, total_pages)
}

/// Core execution helper for `run_merge` (testable without a Tauri `AppHandle`).
pub fn execute_merge_op(
    inputs: &[PathBuf],
    out: Option<&Path>,
    force: bool,
    progress: &dyn Progress,
) -> Result<MergeReport, SheafError> {
    let first = inputs.first().ok_or(SheafError::EmptySelection)?;
    let out_path = match out {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => default_merged_path(first),
    };
    let opts = MergeOptions {
        force,
        dry_run: false,
    };
    merge_with_options(inputs, &out_path, &opts, progress)
}

/// Core execution helper for `run_split` (testable without a Tauri `AppHandle`).
#[allow(clippy::too_many_arguments)]
pub fn execute_split_op(
    input: &Path,
    mode: &str,
    ranges: Option<&str>,
    combine: bool,
    every: Option<u32>,
    bookmarks_level: Option<u8>,
    out_dir: Option<&Path>,
    force: bool,
    progress: &dyn Progress,
) -> Result<SplitReport, SheafError> {
    let split_mode = match mode {
        "ranges" => {
            let spec = ranges.ok_or(SheafError::EmptySelection)?;
            let selection = parse_selection_for_file(input, spec)?;
            SplitMode::Ranges { selection, combine }
        }
        "every" => {
            let n = every.ok_or_else(|| {
                SheafError::InvalidSelection("Missing chunk size N for split every N".to_string())
            })?;
            SplitMode::EveryN(n)
        }
        "bookmarks" => {
            let level = bookmarks_level.unwrap_or(1);
            SplitMode::ByBookmarks { level }
        }
        other => {
            return Err(SheafError::InvalidSelection(format!(
                "Unsupported split mode '{other}'"
            )));
        }
    };

    let target_dir = out_dir.map(Path::to_path_buf).unwrap_or_default();
    let opts = SplitOptions {
        mode: split_mode,
        out_dir: target_dir,
        force,
        dry_run: false,
    };
    split(input, &opts, progress)
}

/// Core execution helper for `run_pages` (testable without a Tauri `AppHandle`).
pub fn execute_pages_op(
    input: &Path,
    action: &str,
    pages: Option<&str>,
    degrees: Option<i32>,
    out: Option<&Path>,
    force: bool,
    progress: &dyn Progress,
) -> Result<PagesReport, SheafError> {
    let opts = PageOpOptions {
        out: out
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf),
        force,
        dry_run: false,
    };

    match action {
        "rotate" => {
            let selection = match pages.map(str::trim).filter(|s| !s.is_empty()) {
                Some(spec) => Some(parse_selection_for_file(input, spec)?),
                None => None,
            };
            let deg = degrees.unwrap_or(90);
            rotate_pages(input, selection.as_ref(), deg, &opts, progress)
        }
        "delete" => {
            let spec = pages
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or(SheafError::EmptySelection)?;
            let selection = parse_selection_for_file(input, spec)?;
            delete_pages(input, &selection, &opts, progress)
        }
        "reorder" => {
            let spec = pages
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or(SheafError::EmptySelection)?;
            let order = parse_selection_for_file(input, spec)?;
            reorder_pages(input, &order, &opts, progress)
        }
        "extract" => {
            let spec = pages
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or(SheafError::EmptySelection)?;
            let selection = parse_selection_for_file(input, spec)?;
            extract_pages(input, &selection, &opts, progress)
        }
        other => Err(SheafError::InvalidSelection(format!(
            "Unsupported page action '{other}'"
        ))),
    }
}

/// Core execution helper for `run_compress` (testable without a Tauri `AppHandle`).
pub fn execute_compress_op(
    input: &Path,
    preset: &str,
    grayscale: bool,
    out: Option<&Path>,
    force: bool,
    progress: &dyn Progress,
) -> Result<CompressReport, SheafError> {
    let parsed_preset = CompressPreset::parse(preset)?;
    let opts = CompressOptions {
        preset: parsed_preset,
        grayscale,
        out: out
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf),
        force,
        dry_run: false,
    };
    compress_with_options(input, &opts, progress)
}

/// Core execution helper for `run_convert` (testable without a Tauri `AppHandle`).
#[allow(clippy::too_many_arguments)]
pub fn execute_convert_op(
    action: &str,
    inputs: &[PathBuf],
    format: Option<&str>,
    dpi: Option<u32>,
    quality: Option<u8>,
    pages: Option<&str>,
    page_size: Option<&str>,
    margin_pt: Option<f32>,
    out: Option<&Path>,
    force: bool,
    progress: &dyn Progress,
) -> Result<ConvertReport, SheafError> {
    let clean_out = out
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf);
    let clean_pages = pages
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    match action {
        "to-images" => {
            let src = inputs.first().ok_or(SheafError::EmptySelection)?;
            let fmt = ImageFormat::parse(format.unwrap_or("png"))?;
            let opts = PdfToImagesOptions {
                format: fmt,
                dpi: dpi.unwrap_or(150),
                quality: quality.unwrap_or(85),
                pages: clean_pages,
                out_dir: clean_out,
                force,
                dry_run: false,
            };
            Ok(ConvertReport::ToImages(pdf_to_images(
                src, &opts, progress,
            )?))
        }
        "from-images" => {
            if inputs.is_empty() {
                return Err(SheafError::EmptySelection);
            }
            let target_out = match clean_out {
                Some(p) => p,
                None => {
                    let first = &inputs[0];
                    let parent = first.parent().unwrap_or_else(|| Path::new("."));
                    parent.join("images_combined.pdf")
                }
            };
            let ps = ConvertPageSize::parse(page_size.unwrap_or("fit"))?;
            let opts = ImagesToPdfOptions {
                page_size: ps,
                margin_pt: margin_pt.unwrap_or(0.0),
                out: target_out,
                force,
                dry_run: false,
            };
            Ok(ConvertReport::FromImages(images_to_pdf(
                inputs, &opts, progress,
            )?))
        }
        "to-markdown" => {
            let src = inputs.first().ok_or(SheafError::EmptySelection)?;
            let opts = PdfToMarkdownOptions {
                pages: clean_pages,
                out: clean_out,
                force,
                dry_run: false,
            };
            Ok(ConvertReport::ToMarkdown(pdf_to_markdown(
                src, &opts, progress,
            )?))
        }
        "from-markdown" => {
            let src = inputs.first().ok_or(SheafError::EmptySelection)?;
            let ps = ConvertPageSize::parse(page_size.unwrap_or("a4"))?;
            let opts = MarkdownToPdfOptions {
                page_size: ps,
                margin_pt: margin_pt.unwrap_or(54.0),
                out: clean_out,
                force,
                dry_run: false,
            };
            Ok(ConvertReport::FromMarkdown(markdown_to_pdf(
                src, &opts, progress,
            )?))
        }
        other => Err(SheafError::InvalidSelection(format!(
            "Unsupported convert action '{other}'"
        ))),
    }
}

/// Core execution helper for `run_watermark` (testable without a Tauri `AppHandle`).
#[allow(clippy::too_many_arguments)]
pub fn execute_watermark_op(
    input: &Path,
    text: &str,
    position: &str,
    font_size: f32,
    opacity: f32,
    rotation_deg: Option<f32>,
    color: &str,
    pages: Option<&str>,
    out: Option<&Path>,
    force: bool,
    progress: &dyn Progress,
) -> Result<WatermarkReport, SheafError> {
    let pos = WatermarkPosition::parse(position)?;
    let opts = WatermarkOptions {
        text: text.to_string(),
        position: pos,
        font_size,
        opacity,
        rotation_deg,
        color: color.to_string(),
        pages: pages
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        out: out
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf),
        force,
        dry_run: false,
    };
    watermark(input, &opts, progress)
}

/// Core execution helper for `run_number` (testable without a Tauri `AppHandle`).
#[allow(clippy::too_many_arguments)]
pub fn execute_number_op(
    input: &Path,
    format: &str,
    position: &str,
    start: u32,
    font_size: f32,
    opacity: f32,
    color: &str,
    margin: f32,
    pages: Option<&str>,
    out: Option<&Path>,
    force: bool,
    progress: &dyn Progress,
) -> Result<NumberReport, SheafError> {
    let pos = NumberPosition::parse(position)?;
    let opts = NumberOptions {
        format: format.to_string(),
        position: pos,
        start,
        font_size,
        opacity,
        color: color.to_string(),
        margin,
        pages: pages
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        out: out
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf),
        force,
        dry_run: false,
    };
    number(input, &opts, progress)
}

/// Core execution helper for `run_protect` (testable without a Tauri `AppHandle`).
#[allow(clippy::too_many_arguments)]
pub fn execute_protect_op(
    action: &str,
    input: &Path,
    password: &str,
    owner_password: Option<&str>,
    allow_print: bool,
    allow_copy: bool,
    out: Option<&Path>,
    force: bool,
    progress: &dyn Progress,
) -> Result<ProtectReport, SheafError> {
    let clean_out = out
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf);

    match action {
        "protect" => {
            let opts = ProtectOptions {
                user_password: secrecy::SecretString::from(password.to_string()),
                owner_password: owner_password
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|s| secrecy::SecretString::from(s.to_string())),
                allow_print,
                allow_copy,
                allow_modify: true,
                out: clean_out,
                force,
                dry_run: false,
            };
            protect(input, &opts, progress)
        }
        "unlock" => {
            let opts = UnlockOptions {
                password: secrecy::SecretString::from(password.to_string()),
                out: clean_out,
                force,
                dry_run: false,
            };
            unlock(input, &opts, progress)
        }
        other => Err(SheafError::InvalidSelection(format!(
            "Unsupported protect action '{other}'"
        ))),
    }
}

/// Inspects one or more local PDF files and returns their [`InfoReport`].
#[tauri::command]
async fn open_pdf(paths: Vec<String>) -> Result<InfoReport, CommandError> {
    let path_bufs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let res = tauri::async_runtime::spawn_blocking(move || info(&path_bufs, &NoProgress))
        .await
        .map_err(|e| CommandError {
            code: "internal",
            message: format!("Worker thread failed: {e}"),
            hint: None,
        })?;
    res.map_err(CommandError::from)
}

/// Renders page thumbnails on a worker thread, utilizing the on-disk thumbnail cache.
#[tauri::command]
async fn thumbnails(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    pages: Vec<u32>,
    width_px: Option<u32>,
) -> Result<Vec<ThumbnailItem>, CommandError> {
    state.cancelled.store(false, Ordering::SeqCst);
    let cancelled = Arc::clone(&state.cancelled);
    let target_width = width_px.unwrap_or(DEFAULT_THUMBNAIL_WIDTH_PX);
    let src_path = PathBuf::from(path);

    let res =
        tauri::async_runtime::spawn_blocking(move || -> Result<Vec<ThumbnailItem>, SheafError> {
            let progress = TauriProgress {
                app,
                op: "thumbnails",
                cancelled,
            };
            let pngs = render_thumbnails(&src_path, &pages, target_width, None, &progress)?;
            let mut items = Vec::with_capacity(pngs.len());
            for (&page, bytes) in pages.iter().zip(pngs) {
                let (w, h) = parse_png_dimensions(&bytes);
                let b64 = BASE64_STANDARD.encode(&bytes);
                items.push(ThumbnailItem {
                    page,
                    width_px: w,
                    height_px: h,
                    data_url: format!("data:image/png;base64,{b64}"),
                });
            }
            Ok(items)
        })
        .await
        .map_err(|e| CommandError {
            code: "internal",
            message: format!("Worker thread failed: {e}"),
            hint: None,
        })?;

    res.map_err(CommandError::from)
}

/// Runs PDF merge on a worker thread with progress reporting and cancellation support.
#[tauri::command]
async fn run_merge(
    app: AppHandle,
    state: State<'_, AppState>,
    inputs: Vec<String>,
    out: Option<String>,
    force: bool,
) -> Result<MergeReport, CommandError> {
    state.cancelled.store(false, Ordering::SeqCst);
    let cancelled = Arc::clone(&state.cancelled);
    let input_paths: Vec<PathBuf> = inputs.into_iter().map(PathBuf::from).collect();
    let out_path = out.map(PathBuf::from);

    let res = tauri::async_runtime::spawn_blocking(move || {
        let progress = TauriProgress {
            app,
            op: "merge",
            cancelled,
        };
        execute_merge_op(&input_paths, out_path.as_deref(), force, &progress)
    })
    .await
    .map_err(|e| CommandError {
        code: "internal",
        message: format!("Worker thread failed: {e}"),
        hint: None,
    })?;

    res.map_err(CommandError::from)
}

/// Runs PDF split on a worker thread with progress reporting and cancellation support.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn run_split(
    app: AppHandle,
    state: State<'_, AppState>,
    input: String,
    mode: String,
    ranges: Option<String>,
    combine: bool,
    every: Option<u32>,
    bookmarks_level: Option<u8>,
    out_dir: Option<String>,
    force: bool,
) -> Result<SplitReport, CommandError> {
    state.cancelled.store(false, Ordering::SeqCst);
    let cancelled = Arc::clone(&state.cancelled);
    let input_path = PathBuf::from(input);
    let target_dir = out_dir.map(PathBuf::from);

    let res = tauri::async_runtime::spawn_blocking(move || {
        let progress = TauriProgress {
            app,
            op: "split",
            cancelled,
        };
        execute_split_op(
            &input_path,
            &mode,
            ranges.as_deref(),
            combine,
            every,
            bookmarks_level,
            target_dir.as_deref(),
            force,
            &progress,
        )
    })
    .await
    .map_err(|e| CommandError {
        code: "internal",
        message: format!("Worker thread failed: {e}"),
        hint: None,
    })?;

    res.map_err(CommandError::from)
}

/// Runs a page-level operation (`rotate`, `delete`, `reorder`, `extract`) on a worker thread.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn run_pages(
    app: AppHandle,
    state: State<'_, AppState>,
    input: String,
    action: String,
    pages: Option<String>,
    degrees: Option<i32>,
    out: Option<String>,
    force: bool,
) -> Result<PagesReport, CommandError> {
    state.cancelled.store(false, Ordering::SeqCst);
    let cancelled = Arc::clone(&state.cancelled);
    let input_path = PathBuf::from(input);
    let out_path = out.map(PathBuf::from);

    let res = tauri::async_runtime::spawn_blocking(move || {
        let progress = TauriProgress {
            app,
            op: "pages",
            cancelled,
        };
        execute_pages_op(
            &input_path,
            &action,
            pages.as_deref(),
            degrees,
            out_path.as_deref(),
            force,
            &progress,
        )
    })
    .await
    .map_err(|e| CommandError {
        code: "internal",
        message: format!("Worker thread failed: {e}"),
        hint: None,
    })?;

    res.map_err(CommandError::from)
}

/// Runs PDF compression on a worker thread with progress reporting and cancellation support.
#[tauri::command]
async fn run_compress(
    app: AppHandle,
    state: State<'_, AppState>,
    input: String,
    preset: String,
    grayscale: bool,
    out: Option<String>,
    force: bool,
) -> Result<CompressReport, CommandError> {
    state.cancelled.store(false, Ordering::SeqCst);
    let cancelled = Arc::clone(&state.cancelled);
    let input_path = PathBuf::from(input);
    let out_path = out.map(PathBuf::from);

    let res = tauri::async_runtime::spawn_blocking(move || {
        let progress = TauriProgress {
            app,
            op: "compress",
            cancelled,
        };
        execute_compress_op(
            &input_path,
            &preset,
            grayscale,
            out_path.as_deref(),
            force,
            &progress,
        )
    })
    .await
    .map_err(|e| CommandError {
        code: "internal",
        message: format!("Worker thread failed: {e}"),
        hint: None,
    })?;

    res.map_err(CommandError::from)
}

/// Runs format conversion (`to-images`, `from-images`, `to-markdown`, `from-markdown`) on a worker thread.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn run_convert(
    app: AppHandle,
    state: State<'_, AppState>,
    action: String,
    inputs: Vec<String>,
    format: Option<String>,
    dpi: Option<u32>,
    quality: Option<u8>,
    pages: Option<String>,
    page_size: Option<String>,
    margin_pt: Option<f32>,
    out: Option<String>,
    force: bool,
) -> Result<ConvertReport, CommandError> {
    state.cancelled.store(false, Ordering::SeqCst);
    let cancelled = Arc::clone(&state.cancelled);
    let input_paths: Vec<PathBuf> = inputs.into_iter().map(PathBuf::from).collect();
    let out_path = out.map(PathBuf::from);

    let res = tauri::async_runtime::spawn_blocking(move || {
        let progress = TauriProgress {
            app,
            op: "convert",
            cancelled,
        };
        execute_convert_op(
            &action,
            &input_paths,
            format.as_deref(),
            dpi,
            quality,
            pages.as_deref(),
            page_size.as_deref(),
            margin_pt,
            out_path.as_deref(),
            force,
            &progress,
        )
    })
    .await
    .map_err(|e| CommandError {
        code: "internal",
        message: format!("Worker thread failed: {e}"),
        hint: None,
    })?;

    res.map_err(CommandError::from)
}

/// Runs PDF watermarking on a worker thread with progress reporting and cancellation support.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn run_watermark(
    app: AppHandle,
    state: State<'_, AppState>,
    input: String,
    text: String,
    position: String,
    font_size: f32,
    opacity: f32,
    rotation_deg: Option<f32>,
    color: String,
    pages: Option<String>,
    out: Option<String>,
    force: bool,
) -> Result<WatermarkReport, CommandError> {
    state.cancelled.store(false, Ordering::SeqCst);
    let cancelled = Arc::clone(&state.cancelled);
    let input_path = PathBuf::from(input);
    let out_path = out.map(PathBuf::from);

    let res = tauri::async_runtime::spawn_blocking(move || {
        let progress = TauriProgress {
            app,
            op: "watermark",
            cancelled,
        };
        execute_watermark_op(
            &input_path,
            &text,
            &position,
            font_size,
            opacity,
            rotation_deg,
            &color,
            pages.as_deref(),
            out_path.as_deref(),
            force,
            &progress,
        )
    })
    .await
    .map_err(|e| CommandError {
        code: "internal",
        message: format!("Worker thread failed: {e}"),
        hint: None,
    })?;

    res.map_err(CommandError::from)
}

/// Runs PDF page numbering on a worker thread with progress reporting and cancellation support.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn run_number(
    app: AppHandle,
    state: State<'_, AppState>,
    input: String,
    format: String,
    position: String,
    start: u32,
    font_size: f32,
    opacity: f32,
    color: String,
    margin: f32,
    pages: Option<String>,
    out: Option<String>,
    force: bool,
) -> Result<NumberReport, CommandError> {
    state.cancelled.store(false, Ordering::SeqCst);
    let cancelled = Arc::clone(&state.cancelled);
    let input_path = PathBuf::from(input);
    let out_path = out.map(PathBuf::from);

    let res = tauri::async_runtime::spawn_blocking(move || {
        let progress = TauriProgress {
            app,
            op: "number",
            cancelled,
        };
        execute_number_op(
            &input_path,
            &format,
            &position,
            start,
            font_size,
            opacity,
            &color,
            margin,
            pages.as_deref(),
            out_path.as_deref(),
            force,
            &progress,
        )
    })
    .await
    .map_err(|e| CommandError {
        code: "internal",
        message: format!("Worker thread failed: {e}"),
        hint: None,
    })?;

    res.map_err(CommandError::from)
}

/// Renders an in-memory live preview of a single PDF page with either a watermark or page number overlay.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn preview_overlay(
    input: String,
    page: u32,
    total_pages: u32,
    mode: String,
    text: String,
    position: String,
    start: u32,
    font_size: f32,
    opacity: f32,
    color: String,
    margin: f32,
    width_px: Option<u32>,
) -> Result<ThumbnailItem, CommandError> {
    let input_path = PathBuf::from(input);
    let target_width = width_px.unwrap_or(240);

    let res = tauri::async_runtime::spawn_blocking(move || -> Result<ThumbnailItem, SheafError> {
        let pos = OverlayPosition::parse(&position)?;
        let (resolved_text, bold, rot) = if mode == "number" {
            let expanded =
                format_page_number(&text, start, total_pages.max(1), page, total_pages.max(1))?;
            (expanded, false, 0.0)
        } else {
            (text, true, pos.default_rotation_deg())
        };
        let png_bytes = preview_overlay_page(
            &input_path,
            page,
            &resolved_text,
            pos,
            font_size,
            opacity,
            rot,
            &color,
            margin,
            bold,
            target_width,
        )?;
        let (w, h) = parse_png_dimensions(&png_bytes);
        let b64 = BASE64_STANDARD.encode(&png_bytes);
        Ok(ThumbnailItem {
            page,
            width_px: w,
            height_px: h,
            data_url: format!("data:image/png;base64,{b64}"),
        })
    })
    .await
    .map_err(|e| CommandError {
        code: "internal",
        message: format!("Worker thread failed: {e}"),
        hint: None,
    })?;

    res.map_err(CommandError::from)
}

/// Runs PDF password protection (`protect`) or decryption (`unlock`) on a worker thread.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn run_protect(
    app: AppHandle,
    state: State<'_, AppState>,
    action: String,
    input: String,
    password: String,
    owner_password: Option<String>,
    allow_print: bool,
    allow_copy: bool,
    out: Option<String>,
    force: bool,
) -> Result<ProtectReport, CommandError> {
    state.cancelled.store(false, Ordering::SeqCst);
    let cancelled = Arc::clone(&state.cancelled);
    let input_path = PathBuf::from(input);
    let out_path = out.map(PathBuf::from);

    let res = tauri::async_runtime::spawn_blocking(move || {
        let progress = TauriProgress {
            app,
            op: "protect",
            cancelled,
        };
        execute_protect_op(
            &action,
            &input_path,
            &password,
            owner_password.as_deref(),
            allow_print,
            allow_copy,
            out_path.as_deref(),
            force,
            &progress,
        )
    })
    .await
    .map_err(|e| CommandError {
        code: "internal",
        message: format!("Worker thread failed: {e}"),
        hint: None,
    })?;

    res.map_err(CommandError::from)
}

/// Reveals a generated file or directory in macOS Finder (`open -R`).
#[tauri::command]
fn reveal_in_finder(path: String) -> Result<(), CommandError> {
    let target = PathBuf::from(&path);
    if !target.exists() {
        return Err(CommandError::from(SheafError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Path does not exist: {path}"),
        ))));
    }

    let status = if target.is_dir() {
        StdCommand::new("open").arg(&target).status()
    } else {
        StdCommand::new("open").arg("-R").arg(&target).status()
    };

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(CommandError {
            code: "io",
            message: format!("Finder exited with status {s}"),
            hint: None,
        }),
        Err(e) => Err(CommandError::from(SheafError::Io(e))),
    }
}

/// Opens a generated PDF file or directory using macOS default application (`open`).
#[tauri::command]
fn open_path(path: String) -> Result<(), CommandError> {
    let target = PathBuf::from(&path);
    if !target.exists() {
        return Err(CommandError::from(SheafError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Path does not exist: {path}"),
        ))));
    }

    let status = StdCommand::new("open").arg(&target).status();
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(CommandError {
            code: "io",
            message: format!("open exited with status {s}"),
            hint: None,
        }),
        Err(e) => Err(CommandError::from(SheafError::Io(e))),
    }
}

/// Signals cancellation to the currently running PDF operation.
#[tauri::command]
fn cancel_op(state: State<'_, AppState>) {
    state.cancelled.store(true, Ordering::SeqCst);
}

/// Returns the [`CapabilitiesReport`] from `sheaf-core`.
#[tauri::command]
fn get_capabilities() -> CapabilitiesReport {
    capabilities()
}

/// Starts the Tauri application.
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            open_pdf,
            thumbnails,
            run_merge,
            run_split,
            run_pages,
            run_compress,
            run_convert,
            run_watermark,
            run_number,
            preview_overlay,
            run_protect,
            reveal_in_finder,
            open_path,
            cancel_op,
            get_capabilities,
        ])
        .run(tauri::generate_context!());

    if let Err(err) = result {
        eprintln!("Failed to run Sheaf desktop application: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct CancelProgress {
        cancelled: Arc<AtomicBool>,
    }

    impl Progress for CancelProgress {
        fn report(&self, _done: u64, _total: u64) {}

        fn is_cancelled(&self) -> bool {
            self.cancelled.load(Ordering::SeqCst)
        }
    }

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .map(|p| p.join("tests").join("fixtures"))
            .unwrap_or_default()
    }

    #[test]
    fn command_error_maps_from_sheaf_error() {
        let err = CommandError::from(SheafError::Encrypted);
        assert_eq!(err.code, "encrypted");
        assert!(err.hint.is_some());
    }

    #[test]
    fn capabilities_command_returns_valid_schema() {
        let cap = get_capabilities();
        assert_eq!(cap.schema, 1);
        assert!(cap.commands.iter().any(|c| c.name == "render"));
    }

    #[test]
    fn desktop_merge_split_pages_end_to_end_and_cancellation() {
        let fixtures = fixtures_dir();
        let f2 = fixtures.join("fixture_a_2p.pdf");
        let f3 = fixtures.join("fixture_b_3p.pdf");
        let temp = tempfile::tempdir().unwrap_or_else(|_| std::process::abort());

        // 1. Merge end-to-end
        let merged_out = temp.path().join("ui_merged.pdf");
        let merge_rep = execute_merge_op(
            &[f2.clone(), f3.clone()],
            Some(&merged_out),
            true,
            &NoProgress,
        )
        .unwrap_or_else(|_| std::process::abort());
        assert_eq!(merge_rep.pages_total, 5);
        assert!(merged_out.exists());

        // 2. Split end-to-end
        let split_dir = temp.path().join("ui_split");
        let split_rep = execute_split_op(
            &merged_out,
            "ranges",
            Some("1-2, 3-5"),
            false,
            None,
            None,
            Some(&split_dir),
            true,
            &NoProgress,
        )
        .unwrap_or_else(|_| std::process::abort());
        assert_eq!(split_rep.outputs.len(), 2);
        assert_eq!(split_rep.outputs[0].pages, 2);
        assert_eq!(split_rep.outputs[1].pages, 3);

        // 3. Pages (rotate, delete, reorder, extract) end-to-end
        let rot_out = temp.path().join("ui_rotated.pdf");
        let rot_rep = execute_pages_op(
            &f3,
            "rotate",
            Some("1,3"),
            Some(90),
            Some(&rot_out),
            true,
            &NoProgress,
        )
        .unwrap_or_else(|_| std::process::abort());
        assert_eq!(rot_rep.action, "rotate");
        assert_eq!(rot_rep.output_pages, 3);
        assert!(rot_out.exists());

        let del_out = temp.path().join("ui_deleted.pdf");
        let del_rep = execute_pages_op(
            &f3,
            "delete",
            Some("2"),
            None,
            Some(&del_out),
            true,
            &NoProgress,
        )
        .unwrap_or_else(|_| std::process::abort());
        assert_eq!(del_rep.output_pages, 2);

        let reord_out = temp.path().join("ui_reordered.pdf");
        let reord_rep = execute_pages_op(
            &f3,
            "reorder",
            Some("3,1,2"),
            None,
            Some(&reord_out),
            true,
            &NoProgress,
        )
        .unwrap_or_else(|_| std::process::abort());
        assert_eq!(reord_rep.pages_affected, vec![3, 1, 2]);

        let ext_out = temp.path().join("ui_extracted.pdf");
        let ext_rep = execute_pages_op(
            &f3,
            "extract",
            Some("2-3"),
            None,
            Some(&ext_out),
            true,
            &NoProgress,
        )
        .unwrap_or_else(|_| std::process::abort());
        assert_eq!(ext_rep.output_pages, 2);

        // 4. Number operation end-to-end
        let num_out = temp.path().join("ui_numbered.pdf");
        let num_rep = execute_number_op(
            &f3,
            "Page {n} of {total}",
            "bottom-right",
            1,
            12.0,
            1.0,
            "#111827",
            28.0,
            Some("2-3"),
            Some(&num_out),
            true,
            &NoProgress,
        )
        .unwrap_or_else(|_| std::process::abort());
        assert_eq!(num_rep.pages_numbered, vec![2, 3]);
        assert!(num_out.exists());

        // 5. Cancellation works and returns SheafError::Cancelled
        let cancelled_flag = Arc::new(AtomicBool::new(true));
        let cancel_progress = CancelProgress {
            cancelled: cancelled_flag,
        };
        let cancel_res = execute_split_op(
            &merged_out,
            "every",
            None,
            false,
            Some(1),
            None,
            Some(&temp.path().join("cancelled_out")),
            true,
            &cancel_progress,
        );
        assert!(matches!(cancel_res, Err(SheafError::Cancelled)));
    }
}
