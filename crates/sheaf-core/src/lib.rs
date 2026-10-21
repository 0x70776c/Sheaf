//! Core PDF manipulation library for **Sheaf**.
//!
//! `sheaf-core` provides pure, offline PDF operations without any UI dependencies.

#![deny(clippy::unwrap_used, clippy::expect_used)]
#![warn(missing_docs)]

mod error;
pub mod fs;
pub mod ops;
mod progress;

/// Current JSON output schema version.
pub const SCHEMA_VERSION: u32 = 1;

pub use error::SheafError;
pub use ops::capabilities::{CapabilitiesReport, capabilities};
pub use ops::compress::{
    CompressOptions, CompressPreset, CompressReport, compress, compress_with_options,
};
pub use ops::info::{FileInfo, InfoReport, PdfMetadataInfo, info};
pub use ops::merge::{MergeOptions, MergeReport, merge, merge_with_options};
pub use ops::pages::{
    PageOpOptions, PagesReport, delete_pages, extract_pages, get_effective_page_rotation,
    normalize_rotation_degrees, reorder_pages, rotate_pages,
};
pub use ops::render::{
    DEFAULT_RENDER_DPI, DEFAULT_THUMBNAIL_WIDTH_PX, RenderOptions, RenderReport, RenderScale,
    RenderedPageOutput, default_thumbnail_cache_dir, is_pdfium_available, render_page,
    render_thumbnails, render_with_options, thumbnail_cache_key,
};
pub use ops::split::{PageSelection, SplitMode, SplitOptions, SplitOutput, SplitReport, split};
pub use ops::verify::{VerifyFileResult, VerifyOptions, VerifyReport, verify};
pub use progress::{NoProgress, Progress};
