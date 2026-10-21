//! Tauri 2 desktop application library for **Sheaf**.
//!
//! Acts as a thin IPC bridge between the Svelte 5 frontend and `sheaf-core`.

#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde::Serialize;
use sheaf_core::{
    CapabilitiesReport, DEFAULT_THUMBNAIL_WIDTH_PX, InfoReport, NoProgress, Progress, SheafError,
    capabilities, info, render_thumbnails,
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
    /// Operation identifier (`"thumbnails"`, `"merge"`, `"split"`, etc.).
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
}
