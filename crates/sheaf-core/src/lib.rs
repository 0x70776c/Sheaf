//! Core PDF manipulation library for **Sheaf**.
//!
//! `sheaf-core` provides pure, offline PDF operations without any UI dependencies.

#![deny(clippy::unwrap_used, clippy::expect_used)]
#![warn(missing_docs)]

mod error;
pub mod fs;
pub mod load;
pub mod ops;
mod progress;

/// Current JSON output schema version.
pub const SCHEMA_VERSION: u32 = 1;

pub use error::SheafError;
pub use load::{LoadOptions, LoadedPdf, load_pdf, load_pdf_from_bytes};
pub use ops::capabilities::{
    ArgumentCapability, CapabilitiesReport, CommandCapability, capabilities, default_exit_codes,
};
pub use ops::compress::{
    CompressOptions, CompressPreset, CompressReport, compress, compress_with_options,
};
pub use ops::convert::{
    ConvertPageSize, ConvertReport, ConvertedImagePage, ImageFormat, ImagesToPdfOptions,
    ImagesToPdfReport, MarkdownToPdfOptions, MarkdownToPdfReport, PdfToImagesOptions,
    PdfToImagesReport, PdfToMarkdownOptions, PdfToMarkdownReport, images_to_pdf, markdown_to_pdf,
    pdf_to_images, pdf_to_markdown,
};
pub use ops::info::{
    EncryptionInfo, EncryptionPermissionsInfo, FileInfo, InfoReport, PdfMetadataInfo, info,
    info_with_options,
};
pub use ops::merge::{MergeOptions, MergeReport, merge, merge_with_options};
pub use ops::number::{NumberOptions, NumberPosition, NumberReport, format_page_number, number};
pub use ops::overlay::{
    OverlayPosition, PageBox, PageOverlaySpec, apply_overlay_to_page, encode_win_ansi,
    get_effective_page_box, parse_overlay_color, preview_overlay_page,
};
pub use ops::pages::{
    PageOpOptions, PagesReport, delete_pages, extract_pages, get_effective_page_rotation,
    normalize_rotation_degrees, reorder_pages, rotate_pages,
};
pub use ops::protect::{ProtectOptions, ProtectReport, UnlockOptions, protect, unlock};
pub use ops::render::{
    DEFAULT_RENDER_DPI, DEFAULT_THUMBNAIL_WIDTH_PX, RenderOptions, RenderReport, RenderScale,
    RenderedPageOutput, default_thumbnail_cache_dir, is_pdfium_available, render_page,
    render_pdf_bytes_page, render_thumbnails, render_thumbnails_with_options, render_with_options,
    thumbnail_cache_key,
};
pub use ops::split::{PageSelection, SplitMode, SplitOptions, SplitOutput, SplitReport, split};
pub use ops::verify::{VerifyFileResult, VerifyOptions, VerifyReport, verify};
pub use ops::watermark::{
    WatermarkOptions, WatermarkPosition, WatermarkReport, parse_watermark_color, watermark,
};
pub use progress::{NoProgress, Progress};
