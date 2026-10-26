//! PDF page rendering and thumbnail caching (`render_page`, `render_thumbnail`, `render_thumbnails`, `render_with_options`).

use std::fs;
use std::io::{BufWriter, Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

use image::ImageEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use pdfium_render::prelude::*;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::ensure_not_overwriting_inputs;
use crate::fs::{check_output_path, write_atomic};
use crate::load::LoadOptions;
use crate::ops::split::PageSelection;
use crate::{Progress, SheafError};

/// Default thumbnail width in pixels (`160` px).
pub const DEFAULT_THUMBNAIL_WIDTH_PX: u32 = 160;

/// Default rendering resolution in dots per inch (`150` DPI).
pub const DEFAULT_RENDER_DPI: u32 = 150;

static PDFIUM_INSTANCE: OnceLock<Result<Pdfium, String>> = OnceLock::new();
static PDFIUM_RESOLVED_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

/// PDFium library availability and location status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfiumStatus {
    /// Whether a functional PDFium library was discovered and bound.
    pub found: bool,
    /// Absolute path to the loaded library, if found.
    pub path: Option<String>,
}

/// Probes whether PDFium is available and returns its resolved library path if found.
pub fn get_pdfium_status() -> PdfiumStatus {
    let available = get_pdfium().is_ok();
    let path = PDFIUM_RESOLVED_PATH
        .get()
        .and_then(|p| p.as_ref())
        .map(|p| p.to_string_lossy().to_string());
    PdfiumStatus {
        found: available,
        path,
    }
}

/// Scaling mode for rendering PDF pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderScale {
    /// Target width in pixels (height is scaled proportionally to preserve aspect ratio).
    WidthPx(u32),
    /// Target resolution in dots per inch (1 PDF point = 1/72 inch).
    Dpi(u32),
}

/// Options for [`render_with_options`].
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Loading and decryption options for input PDF.
    pub load: LoadOptions,
    /// 1-based page selection (`None` renders all pages in the document).
    pub pages: Option<String>,
    /// Target scaling mode (pixel width or DPI).
    pub scale: RenderScale,
    /// Output file path (when rendering a single page to `.png`) or output directory.
    pub out: PathBuf,
    /// Overwrite existing output files if `true`.
    pub force: bool,
    /// Compute planned outputs without writing files if `true`.
    pub dry_run: bool,
    /// Use on-disk thumbnail cache when `scale` is [`RenderScale::WidthPx`].
    pub use_cache: bool,
    /// Optional custom directory for the on-disk thumbnail cache.
    pub cache_dir: Option<PathBuf>,
}

/// Metadata for a single rendered page output file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RenderedPageOutput {
    /// 1-based page number in the source document.
    pub page: u32,
    /// Output PNG file path.
    pub path: PathBuf,
    /// Rendered image width in pixels.
    pub width_px: u32,
    /// Rendered image height in pixels.
    pub height_px: u32,
    /// Whether the PNG was served from the on-disk thumbnail cache.
    pub cached: bool,
}

/// Report returned by [`render_with_options`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RenderReport {
    /// Input PDF file path.
    pub input: PathBuf,
    /// Total number of pages in the source PDF document.
    pub total_pages: u32,
    /// List of rendered output files.
    pub outputs: Vec<RenderedPageOutput>,
    /// Indicates whether `--dry-run` was active.
    pub dry_run: bool,
}

/// Returns whether the PDFium dynamic library can be loaded on the current system.
///
/// # Examples
///
/// ```
/// use sheaf_core::ops::render::is_pdfium_available;
///
/// let _available = is_pdfium_available();
/// ```
pub fn is_pdfium_available() -> bool {
    get_pdfium().is_ok()
}

pub(crate) fn get_pdfium() -> Result<&'static Pdfium, SheafError> {
    let res = PDFIUM_INSTANCE.get_or_init(init_pdfium_bindings);
    match res {
        Ok(pdfium) => Ok(pdfium),
        Err(msg) => Err(SheafError::Unsupported(msg.clone())),
    }
}

fn init_pdfium_bindings() -> Result<Pdfium, String> {
    let mut candidate_paths: Vec<PathBuf> = Vec::new();

    // 1. PDFIUM_LIB_PATH environment variable
    for env_key in ["PDFIUM_LIB_PATH", "PDFIUM_DYNAMIC_LIB_PATH"] {
        if let Some(val) = std::env::var_os(env_key) {
            let p = PathBuf::from(val);
            if p.is_file() {
                candidate_paths.push(p);
            } else if p.is_dir() {
                candidate_paths.push(p.join("libpdfium.dylib"));
            }
        }
    }

    // 2. Beside executable, 3. ../lib/, 4. ../Frameworks/ (App-Bundle)
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(exe_dir) = exe_path.parent()
    {
        candidate_paths.push(exe_dir.join("libpdfium.dylib"));
        candidate_paths.push(exe_dir.join("lib/libpdfium.dylib"));
        candidate_paths.push(exe_dir.join("../lib/libpdfium.dylib"));
        candidate_paths.push(exe_dir.join("../Frameworks/libpdfium.dylib"));
        candidate_paths.push(exe_dir.join("../Resources/lib/libpdfium.dylib"));
    }

    // 5. System paths and fallback
    candidate_paths.push(PathBuf::from("/opt/homebrew/lib/libpdfium.dylib"));
    candidate_paths.push(PathBuf::from("/usr/local/lib/libpdfium.dylib"));
    candidate_paths.push(PathBuf::from("/usr/lib/libpdfium.dylib"));
    candidate_paths.push(PathBuf::from("./libpdfium.dylib"));
    candidate_paths.push(PathBuf::from("./vendor/pdfium/lib/libpdfium.dylib"));
    candidate_paths.push(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/pdfium/lib/libpdfium.dylib"),
    );

    let mut probed: Vec<PathBuf> = Vec::new();
    for path in candidate_paths {
        if probed.contains(&path) {
            continue;
        }
        probed.push(path.clone());
        if path.exists()
            && let Ok(bindings) = Pdfium::bind_to_library(&path)
        {
            let _ = PDFIUM_RESOLVED_PATH.set(Some(path));
            return Ok(Pdfium::new(bindings));
        }
    }

    if let Ok(bindings) = Pdfium::bind_to_system_library() {
        let _ = PDFIUM_RESOLVED_PATH.set(None);
        return Ok(Pdfium::new(bindings));
    }

    let _ = PDFIUM_RESOLVED_PATH.set(None);
    let probe_list = probed
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "PDFium library (libpdfium.dylib) not found. Searched candidate locations: [{probe_list}]. Set PDFIUM_LIB_PATH or place libpdfium.dylib beside the executable, in ../lib/, or ../Frameworks/."
    ))
}

fn map_pdfium_error(err: PdfiumError, path: &Path) -> SheafError {
    match err {
        PdfiumError::IoError(io_err) => SheafError::Io(io_err),
        PdfiumError::PdfiumLibraryInternalError(internal) => match internal {
            PdfiumInternalError::FileError => SheafError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Failed to open PDF file: {}", path.display()),
            )),
            PdfiumInternalError::PasswordError | PdfiumInternalError::SecurityError => {
                SheafError::Encrypted
            }
            PdfiumInternalError::FormatError
            | PdfiumInternalError::PageError
            | PdfiumInternalError::Unknown => {
                SheafError::InvalidPdf(format!("{}: {internal:?}", path.display()))
            }
        },
        PdfiumError::PageIndexOutOfBounds => SheafError::PageOutOfRange { page: 0, total: 0 },
        PdfiumError::ImageSizeOutOfBounds => SheafError::InvalidSelection(
            "Requested render dimensions exceed maximum supported bitmap size".to_string(),
        ),
        other => SheafError::InvalidPdf(format!("{}: {other}", path.display())),
    }
}

fn map_pdfium_load_error(err: PdfiumError, path: &Path, had_password: bool) -> SheafError {
    if let PdfiumError::PdfiumLibraryInternalError(
        PdfiumInternalError::PasswordError | PdfiumInternalError::SecurityError,
    ) = &err
    {
        if had_password {
            return SheafError::WrongPassword;
        } else {
            return SheafError::Encrypted;
        }
    }
    map_pdfium_error(err, path)
}

fn validate_width_px(width_px: u32) -> Result<i32, SheafError> {
    if width_px == 0 {
        return Err(SheafError::InvalidSelection(
            "Target width must be greater than 0 px".to_string(),
        ));
    }
    if width_px > i32::from(u16::MAX) as u32 {
        return Err(SheafError::InvalidSelection(format!(
            "Target width ({width_px} px) exceeds maximum supported width ({} px)",
            u16::MAX
        )));
    }
    i32::try_from(width_px)
        .map_err(|_| SheafError::InvalidSelection("Invalid target width".to_string()))
}

fn validate_dpi(dpi: u32) -> Result<f32, SheafError> {
    if dpi == 0 {
        return Err(SheafError::InvalidSelection(
            "Target DPI must be greater than 0".to_string(),
        ));
    }
    if dpi > 2400 {
        return Err(SheafError::InvalidSelection(format!(
            "Target DPI ({dpi}) exceeds maximum supported DPI (2400)"
        )));
    }
    Ok(dpi as f32 / 72.0)
}

fn encode_bitmap_to_png(bitmap: &PdfBitmap<'_>) -> Result<(Vec<u8>, u32, u32), SheafError> {
    let dyn_image = bitmap.as_image().map_err(|e| {
        SheafError::InvalidPdf(format!("Failed to convert rendered bitmap to image: {e}"))
    })?;
    let rgba = dyn_image.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();

    let mut png_bytes = Vec::with_capacity((width as usize * height as usize) / 2 + 1024);
    let encoder = PngEncoder::new_with_quality(
        Cursor::new(&mut png_bytes),
        CompressionType::Fast,
        FilterType::Sub,
    );
    encoder
        .write_image(
            rgba.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| SheafError::InvalidPdf(format!("Failed to encode PNG: {e}")))?;

    Ok((png_bytes, width, height))
}

fn read_png_dimensions(png_bytes: &[u8]) -> Option<(u32, u32)> {
    // PNG header: 8-byte signature + 4-byte chunk length + 4-byte "IHDR" + 4-byte width + 4-byte height
    if png_bytes.len() < 24
        || &png_bytes[0..8] != b"\x89PNG\r\n\x1a\n"
        || &png_bytes[12..16] != b"IHDR"
    {
        return None;
    }
    let width = u32::from_be_bytes([png_bytes[16], png_bytes[17], png_bytes[18], png_bytes[19]]);
    let height = u32::from_be_bytes([png_bytes[20], png_bytes[21], png_bytes[22], png_bytes[23]]);
    Some((width, height))
}

/// Computes the deterministic SHA-256 disk-cache key for a rendered page thumbnail.
///
/// The key incorporates the canonical file path, modification timestamp (`mtime`), file size,
/// 1-based page number, and target pixel width so any modification to the file automatically
/// invalidates stale cache entries.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use sheaf_core::ops::render::thumbnail_cache_key;
///
/// let key = thumbnail_cache_key(Path::new("input.pdf"), 1, 160).unwrap();
/// assert_eq!(key.len(), 64);
/// ```
pub fn thumbnail_cache_key(src: &Path, page: u32, width_px: u32) -> Result<String, SheafError> {
    let meta = fs::metadata(src)?;
    let canonical = fs::canonicalize(src).unwrap_or_else(|_| src.to_path_buf());
    let (mtime_secs, mtime_nanos) = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| (d.as_secs(), d.subsec_nanos()))
        .unwrap_or((0, 0));

    let mut hasher = Sha256::new();
    hasher.update(canonical.to_string_lossy().as_bytes());
    hasher.update(b"|");
    hasher.update(mtime_secs.to_le_bytes());
    hasher.update(mtime_nanos.to_le_bytes());
    hasher.update(meta.len().to_le_bytes());
    hasher.update(page.to_le_bytes());
    hasher.update(width_px.to_le_bytes());

    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut hex, "{byte:02x}");
    }
    Ok(hex)
}

/// Returns the default directory used for caching rendered thumbnails on disk.
///
/// # Examples
///
/// ```
/// use sheaf_core::ops::render::default_thumbnail_cache_dir;
///
/// let dir = default_thumbnail_cache_dir();
/// assert!(dir.ends_with("sheaf-thumbnails"));
/// ```
pub fn default_thumbnail_cache_dir() -> PathBuf {
    std::env::temp_dir().join("sheaf-thumbnails")
}

/// Renders a single 1-based page of a PDF document to PNG bytes at the specified pixel width.
///
/// Aspect ratio is preserved automatically.
///
/// # Errors
///
/// Returns:
/// - [`SheafError::Io`] if `src` does not exist or cannot be read.
/// - [`SheafError::Encrypted`] if `src` is password-protected.
/// - [`SheafError::InvalidPdf`] if `src` is corrupted or not a valid PDF.
/// - [`SheafError::PageOutOfRange`] if `page == 0` or `page > total_pages`.
/// - [`SheafError::InvalidSelection`] if `width_px == 0` or exceeds `65535`.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use sheaf_core::render_page;
///
/// let png_bytes = render_page(Path::new("document.pdf"), 1, 160).unwrap();
/// assert!(png_bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
/// ```
pub fn render_page(src: &Path, page: u32, width_px: u32) -> Result<Vec<u8>, SheafError> {
    let target_width = validate_width_px(width_px)?;
    let _meta = fs::metadata(src)?;

    let pdfium = get_pdfium()?;
    let doc = pdfium
        .load_pdf_from_file(src, None)
        .map_err(|e| map_pdfium_load_error(e, src, false))?;

    let total = u32::try_from(doc.pages().len()).unwrap_or(0);
    if page == 0 || page > total {
        return Err(SheafError::PageOutOfRange { page, total });
    }

    let page_index =
        i32::try_from(page - 1).map_err(|_| SheafError::PageOutOfRange { page, total })?;
    let pdf_page = doc
        .pages()
        .get(page_index)
        .map_err(|e| map_pdfium_error(e, src))?;

    let config = PdfRenderConfig::new().set_target_width(target_width);
    let bitmap = pdf_page
        .render_with_config(&config)
        .map_err(|e| map_pdfium_error(e, src))?;

    let (png_bytes, _, _) = encode_bitmap_to_png(&bitmap)?;
    Ok(png_bytes)
}

/// Renders a 1-based page from in-memory PDF bytes to PNG bytes at `width_px` target width.
///
/// # Examples
///
/// ```no_run
/// use sheaf_core::ops::render::render_pdf_bytes_page;
///
/// let bytes = std::fs::read("document.pdf").unwrap();
/// let png = render_pdf_bytes_page(&bytes, 1, 200).unwrap();
/// assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
/// ```
pub fn render_pdf_bytes_page(
    pdf_bytes: &[u8],
    page: u32,
    width_px: u32,
) -> Result<Vec<u8>, SheafError> {
    let target_width = validate_width_px(width_px)?;
    let pdfium = get_pdfium()?;
    let dummy_path = Path::new("<memory>");
    let doc = pdfium
        .load_pdf_from_byte_slice(pdf_bytes, None)
        .map_err(|e| map_pdfium_error(e, dummy_path))?;

    let total = u32::try_from(doc.pages().len()).unwrap_or(0);
    if page == 0 || page > total {
        return Err(SheafError::PageOutOfRange { page, total });
    }

    let page_index =
        i32::try_from(page - 1).map_err(|_| SheafError::PageOutOfRange { page, total })?;
    let pdf_page = doc
        .pages()
        .get(page_index)
        .map_err(|e| map_pdfium_error(e, dummy_path))?;

    let config = PdfRenderConfig::new().set_target_width(target_width);
    let bitmap = pdf_page
        .render_with_config(&config)
        .map_err(|e| map_pdfium_error(e, dummy_path))?;

    let (png_bytes, _, _) = encode_bitmap_to_png(&bitmap)?;
    Ok(png_bytes)
}

/// Renders a batch of 1-based pages as thumbnails (default `160` px width), utilizing an on-disk
/// PNG cache keyed by `(path, mtime, size, page, width_px)`.
///
/// The PDF document is opened at most once per batch call when uncached pages need rendering,
/// ensuring high throughput on large documents (e.g. 500+ pages).
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use sheaf_core::{NoProgress, render_thumbnails, DEFAULT_THUMBNAIL_WIDTH_PX};
///
/// let pages: Vec<u32> = (1..=20).collect();
/// let pngs = render_thumbnails(
///     Path::new("large.pdf"),
///     &pages,
///     DEFAULT_THUMBNAIL_WIDTH_PX,
///     None,
///     &NoProgress,
/// ).unwrap();
/// assert_eq!(pngs.len(), 20);
/// ```
/// Renders thumbnails for a set of pages in a document.
///
/// Bypasses disk cache and operates RAM-only if password is provided or document is encrypted.
pub fn render_thumbnails_with_options(
    src: &Path,
    pages: &[u32],
    width_px: u32,
    cache_dir: Option<&Path>,
    load: &LoadOptions,
    p: &dyn Progress,
) -> Result<Vec<Vec<u8>>, SheafError> {
    if pages.is_empty() {
        return Err(SheafError::EmptySelection);
    }
    let target_width = validate_width_px(width_px)?;
    let _meta = fs::metadata(src)?;

    let pw_str = load.password.as_ref().map(|s| s.expose_secret());
    let bypass_disk_cache = pw_str.is_some();

    let resolved_cache_dir = if !bypass_disk_cache {
        let dir = cache_dir
            .map(Path::to_path_buf)
            .unwrap_or_else(default_thumbnail_cache_dir);
        let _ = fs::create_dir_all(&dir);
        Some(dir)
    } else {
        None
    };

    let total_requested = u64::try_from(pages.len()).unwrap_or(u64::MAX);
    let mut results: Vec<Option<Vec<u8>>> = vec![None; pages.len()];
    let mut cache_paths: Vec<Option<PathBuf>> = Vec::with_capacity(pages.len());
    let mut missing_indices: Vec<usize> = Vec::new();

    for (idx, &page) in pages.iter().enumerate() {
        if page == 0 {
            // Need document page count for accurate PageOutOfRange error
            let pdfium = get_pdfium()?;
            let doc = pdfium
                .load_pdf_from_file(src, pw_str)
                .map_err(|e| map_pdfium_load_error(e, src, pw_str.is_some()))?;
            let total = u32::try_from(doc.pages().len()).unwrap_or(0);
            return Err(SheafError::PageOutOfRange { page: 0, total });
        }

        if let Some(ref dir) = resolved_cache_dir {
            let key = thumbnail_cache_key(src, page, width_px)?;
            let cache_file = dir.join(format!("{key}.png"));
            if let Ok(bytes) = fs::read(&cache_file)
                && read_png_dimensions(&bytes).is_some()
            {
                results[idx] = Some(bytes);
            } else {
                missing_indices.push(idx);
            }
            cache_paths.push(Some(cache_file));
        } else {
            missing_indices.push(idx);
            cache_paths.push(None);
        }
    }

    if !missing_indices.is_empty() {
        let pdfium = get_pdfium()?;
        let doc = pdfium
            .load_pdf_from_file(src, pw_str)
            .map_err(|e| map_pdfium_load_error(e, src, pw_str.is_some()))?;
        let total_doc_pages = u32::try_from(doc.pages().len()).unwrap_or(0);

        for &page in pages {
            if page == 0 || page > total_doc_pages {
                return Err(SheafError::PageOutOfRange {
                    page,
                    total: total_doc_pages,
                });
            }
        }

        let config = PdfRenderConfig::new().set_target_width(target_width);

        for (step, &idx) in missing_indices.iter().enumerate() {
            if p.is_cancelled() {
                return Err(SheafError::Cancelled);
            }
            let page = pages[idx];
            p.report(u64::try_from(step).unwrap_or(0), total_requested);

            let page_index = i32::try_from(page - 1).map_err(|_| SheafError::PageOutOfRange {
                page,
                total: total_doc_pages,
            })?;
            let pdf_page = doc
                .pages()
                .get(page_index)
                .map_err(|e| map_pdfium_error(e, src))?;
            let bitmap = pdf_page
                .render_with_config(&config)
                .map_err(|e| map_pdfium_error(e, src))?;
            let (png_bytes, _, _) = encode_bitmap_to_png(&bitmap)?;

            // Write atomically to cache (best-effort, only if not bypassing cache)
            if let Some(Some(cache_file)) = cache_paths.get(idx) {
                let _ = write_atomic(cache_file, true, |file| {
                    let mut writer = BufWriter::new(file);
                    writer.write_all(&png_bytes)?;
                    writer.flush()?;
                    Ok(())
                });
            }

            results[idx] = Some(png_bytes);
        }
    }

    p.report(total_requested, total_requested);

    let mut out = Vec::with_capacity(results.len());
    for item in results {
        match item {
            Some(bytes) => out.push(bytes),
            None => {
                return Err(SheafError::InvalidPdf(
                    "Internal error: missing rendered thumbnail".to_string(),
                ));
            }
        }
    }
    Ok(out)
}

/// Renders thumbnails for a set of pages in a document, writing rendered PNGs to an on-disk cache.
///
/// Subsequent calls with identical file content and parameters reuse the cached PNGs.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use sheaf_core::{DEFAULT_THUMBNAIL_WIDTH_PX, NoProgress, render_thumbnails};
///
/// let pages: Vec<u32> = (1..=20).collect();
/// let pngs = render_thumbnails(
///     Path::new("large.pdf"),
///     &pages,
///     DEFAULT_THUMBNAIL_WIDTH_PX,
///     None,
///     &NoProgress,
/// ).unwrap();
/// assert_eq!(pngs.len(), 20);
/// ```
pub fn render_thumbnails(
    src: &Path,
    pages: &[u32],
    width_px: u32,
    cache_dir: Option<&Path>,
    p: &dyn Progress,
) -> Result<Vec<Vec<u8>>, SheafError> {
    render_thumbnails_with_options(src, pages, width_px, cache_dir, &LoadOptions::none(), p)
}

/// Renders one or more PDF pages to PNG files on disk according to [`RenderOptions`].
///
/// Supports rendering by pixel width (`--width`) or resolution (`--dpi`), single-file or
/// directory output, atomic file writes, `--dry-run`, `--force`, and disk caching.
///
/// # Examples
///
/// ```no_run
/// use std::path::{Path, PathBuf};
/// use sheaf_core::{LoadOptions, NoProgress, RenderOptions, RenderScale, render_with_options};
///
/// let opts = RenderOptions {
///     load: LoadOptions::none(),
///     pages: Some("1-2".to_string()),
///     scale: RenderScale::WidthPx(160),
///     out: PathBuf::from("thumbs"),
///     force: true,
///     dry_run: false,
///     use_cache: true,
///     cache_dir: None,
/// };
/// let report = render_with_options(Path::new("input.pdf"), &opts, &NoProgress).unwrap();
/// assert_eq!(report.outputs.len(), 2);
/// ```
pub fn render_with_options(
    src: &Path,
    opts: &RenderOptions,
    p: &dyn Progress,
) -> Result<RenderReport, SheafError> {
    if p.is_cancelled() {
        return Err(SheafError::Cancelled);
    }

    let _meta = fs::metadata(src)?;

    // Validate scale parameters up-front
    match opts.scale {
        RenderScale::WidthPx(w) => {
            let _ = validate_width_px(w)?;
        }
        RenderScale::Dpi(dpi) => {
            let _ = validate_dpi(dpi)?;
        }
    }

    let pw_str = opts.load.password.as_ref().map(|s| s.expose_secret());
    let pdfium = get_pdfium()?;
    let doc = pdfium
        .load_pdf_from_file(src, pw_str)
        .map_err(|e| map_pdfium_load_error(e, src, pw_str.is_some()))?;

    let total_pages = u32::try_from(doc.pages().len()).unwrap_or(0);
    if total_pages == 0 {
        return Err(SheafError::InvalidPdf(format!(
            "{}: document contains 0 pages",
            src.display()
        )));
    }

    let selected_pages: Vec<u32> = match &opts.pages {
        Some(spec) => PageSelection::parse(spec, total_pages)?.to_page_numbers(),
        None => (1..=total_pages).collect(),
    };

    if selected_pages.is_empty() {
        return Err(SheafError::EmptySelection);
    }

    let stem = src
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("document");

    // Determine output file paths:
    // If rendering a single page and `opts.out` has `.png` extension, write directly to `opts.out`.
    // Otherwise treat `opts.out` as an output directory and write `<stem>_p001.png`, etc.
    let single_png_file = selected_pages.len() == 1
        && opts
            .out
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"));

    let pad_width = std::cmp::max(3, total_pages.to_string().len());
    let mut planned_paths: Vec<(u32, PathBuf)> = Vec::with_capacity(selected_pages.len());

    if single_png_file {
        ensure_not_overwriting_inputs(&[src.to_path_buf()], &opts.out)?;
        check_output_path(&opts.out, opts.force)?;
        planned_paths.push((selected_pages[0], opts.out.clone()));
    } else {
        let mut seen = std::collections::HashMap::<u32, usize>::new();
        for &page in &selected_pages {
            let count = seen.entry(page).and_modify(|c| *c += 1).or_insert(1);
            let filename = if *count == 1 {
                format!("{stem}_p{page:0>width$}.png", width = pad_width)
            } else {
                format!("{stem}_p{page:0>width$}_{count}.png", width = pad_width)
            };
            let target_path = opts.out.join(filename);
            ensure_not_overwriting_inputs(&[src.to_path_buf()], &target_path)?;
            check_output_path(&target_path, opts.force)?;
            planned_paths.push((page, target_path));
        }
    }

    let resolved_cache_dir = if opts.use_cache && pw_str.is_none() {
        let dir = opts
            .cache_dir
            .clone()
            .unwrap_or_else(default_thumbnail_cache_dir);
        if !opts.dry_run {
            fs::create_dir_all(&dir)?;
        }
        Some(dir)
    } else {
        None
    };

    let total_steps = u64::try_from(planned_paths.len()).unwrap_or(u64::MAX);
    let mut outputs = Vec::with_capacity(planned_paths.len());

    for (idx, (page, out_path)) in planned_paths.into_iter().enumerate() {
        if p.is_cancelled() {
            return Err(SheafError::Cancelled);
        }

        let step_num = u64::try_from(idx).unwrap_or(0);
        p.report(step_num, total_steps);

        let page_index = i32::try_from(page - 1).map_err(|_| SheafError::PageOutOfRange {
            page,
            total: total_pages,
        })?;

        if opts.dry_run {
            let rect = doc
                .pages()
                .page_size(page_index)
                .map_err(|e| map_pdfium_error(e, src))?;
            let (width_px, height_px) = match opts.scale {
                RenderScale::WidthPx(w) => {
                    let scale = (w as f32) / rect.width().value.max(1.0);
                    let h = (rect.height().value * scale).round().max(1.0) as u32;
                    (w, h)
                }
                RenderScale::Dpi(dpi) => {
                    let scale = (dpi as f32) / 72.0;
                    let w = (rect.width().value * scale).round().max(1.0) as u32;
                    let h = (rect.height().value * scale).round().max(1.0) as u32;
                    (w, h)
                }
            };
            outputs.push(RenderedPageOutput {
                page,
                path: out_path,
                width_px,
                height_px,
                cached: false,
            });
            continue;
        }

        // Check disk cache if rendering by WidthPx and caching is enabled
        let mut cached_hit: Option<(Vec<u8>, u32, u32)> = None;
        let mut cache_file_path: Option<PathBuf> = None;

        if let (RenderScale::WidthPx(w), Some(cache_dir)) = (opts.scale, &resolved_cache_dir) {
            let key = thumbnail_cache_key(src, page, w)?;
            let cache_file = cache_dir.join(format!("{key}.png"));
            if let Ok(bytes) = fs::read(&cache_file)
                && let Some((cw, ch)) = read_png_dimensions(&bytes)
            {
                cached_hit = Some((bytes, cw, ch));
            }
            cache_file_path = Some(cache_file);
        }

        let (png_bytes, width_px, height_px, cached) = match cached_hit {
            Some((bytes, w, h)) => (bytes, w, h, true),
            None => {
                let pdf_page = doc
                    .pages()
                    .get(page_index)
                    .map_err(|e| map_pdfium_error(e, src))?;
                let config = match opts.scale {
                    RenderScale::WidthPx(w) => {
                        let target_w = validate_width_px(w)?;
                        PdfRenderConfig::new().set_target_width(target_w)
                    }
                    RenderScale::Dpi(dpi) => {
                        let factor = validate_dpi(dpi)?;
                        PdfRenderConfig::new().scale_page_by_factor(factor)
                    }
                };
                let bitmap = pdf_page
                    .render_with_config(&config)
                    .map_err(|e| map_pdfium_error(e, src))?;
                let (bytes, w, h) = encode_bitmap_to_png(&bitmap)?;

                if let Some(cache_file) = cache_file_path {
                    let _ = write_atomic(&cache_file, true, |file| {
                        let mut writer = BufWriter::new(file);
                        writer.write_all(&bytes)?;
                        writer.flush()?;
                        Ok(())
                    });
                }

                (bytes, w, h, false)
            }
        };

        write_atomic(&out_path, opts.force, |file| {
            let mut writer = BufWriter::new(file);
            writer.write_all(&png_bytes)?;
            writer.flush()?;
            Ok(())
        })?;

        outputs.push(RenderedPageOutput {
            page,
            path: out_path,
            width_px,
            height_px,
            cached,
        });
    }

    p.report(total_steps, total_steps);

    Ok(RenderReport {
        input: src.to_path_buf(),
        total_pages,
        outputs,
        dry_run: opts.dry_run,
    })
}
