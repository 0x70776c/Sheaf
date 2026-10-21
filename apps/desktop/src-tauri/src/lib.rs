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
    CapabilitiesReport, CompressOptions, CompressPreset, CompressReport,
    DEFAULT_THUMBNAIL_WIDTH_PX, InfoReport, MergeOptions, MergeReport, NoProgress, PageOpOptions,
    PageSelection, PagesReport, Progress, SheafError, SplitMode, SplitOptions, SplitReport,
    capabilities, compress_with_options, delete_pages, extract_pages, info, merge_with_options,
    render_thumbnails, reorder_pages, rotate_pages, split,
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

        // 4. Cancellation works and returns SheafError::Cancelled
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
