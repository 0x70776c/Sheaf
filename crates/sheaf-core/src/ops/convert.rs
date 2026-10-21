//! Format conversion operations:
//! - [`pdf_to_images`]: Export PDF pages as `PNG` or `JPG` images.
//! - [`images_to_pdf`]: Combine `PNG`/`JPG` images into a multi-page PDF.
//! - [`pdf_to_markdown`]: Extract structured Markdown (`.md`) from a PDF document.
//! - [`markdown_to_pdf`]: Render a Markdown (`.md`) document into a paginated vector PDF.

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufWriter, Cursor, Write};
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType as PngFilterType, PngEncoder};
use image::{ColorType, DynamicImage, ExtendedColorType, ImageEncoder, ImageReader};
use lopdf::content::{Content, Operation};
use lopdf::{Bookmark, Dictionary, Document, Object, ObjectId, Stream, dictionary};
use pdfium_render::prelude::*;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};

use crate::error::ensure_not_overwriting_inputs;
use crate::fs::{check_output_path, write_atomic};
use crate::load::{LoadOptions, load_pdf, load_pdf_from_bytes};
use crate::ops::render::get_pdfium;
use crate::ops::split::{PageSelection, sanitize_stem};
use crate::{Progress, SheafError};

/// Target image format for [`pdf_to_images`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    /// Lossless PNG image (`.png`).
    #[default]
    Png,
    /// Compressed JPEG image (`.jpg`).
    Jpg,
}

impl ImageFormat {
    /// Parses an image format name (`"png"`, `"jpg"`, or `"jpeg"`).
    ///
    /// # Examples
    ///
    /// ```
    /// use sheaf_core::ImageFormat;
    ///
    /// assert_eq!(ImageFormat::parse("png").unwrap(), ImageFormat::Png);
    /// assert_eq!(ImageFormat::parse("jpeg").unwrap(), ImageFormat::Jpg);
    /// assert!(ImageFormat::parse("bmp").is_err());
    /// ```
    pub fn parse(input: &str) -> Result<Self, SheafError> {
        match input.trim().to_ascii_lowercase().as_str() {
            "png" => Ok(Self::Png),
            "jpg" | "jpeg" => Ok(Self::Jpg),
            other => Err(SheafError::InvalidSelection(format!(
                "Invalid image format '{other}' (expected: png or jpg)"
            ))),
        }
    }

    /// File extension without leading dot (`"png"` or `"jpg"`).
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpg => "jpg",
        }
    }
}

/// Target page size for [`images_to_pdf`] and [`markdown_to_pdf`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConvertPageSize {
    /// Match each page's dimensions to the source image size at 72 DPI (for [`images_to_pdf`]).
    #[default]
    Fit,
    /// ISO A4 portrait (`595.28 × 841.89 pt`).
    A4,
    /// US Letter portrait (`612.0 × 792.0 pt`).
    Letter,
}

impl ConvertPageSize {
    /// Parses a page size name (`"fit"`, `"a4"`, `"letter"`).
    ///
    /// # Examples
    ///
    /// ```
    /// use sheaf_core::ConvertPageSize;
    ///
    /// assert_eq!(ConvertPageSize::parse("a4").unwrap(), ConvertPageSize::A4);
    /// assert_eq!(ConvertPageSize::parse("fit").unwrap(), ConvertPageSize::Fit);
    /// assert!(ConvertPageSize::parse("tabloid").is_err());
    /// ```
    pub fn parse(input: &str) -> Result<Self, SheafError> {
        match input.trim().to_ascii_lowercase().as_str() {
            "fit" => Ok(Self::Fit),
            "a4" => Ok(Self::A4),
            "letter" => Ok(Self::Letter),
            other => Err(SheafError::InvalidSelection(format!(
                "Invalid page size '{other}' (expected: fit, a4, or letter)"
            ))),
        }
    }

    /// Stable lowercase identifier (`"fit"`, `"a4"`, `"letter"`).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fit => "fit",
            Self::A4 => "a4",
            Self::Letter => "letter",
        }
    }

    /// Returns fixed `(width_pt, height_pt)` dimensions (`Fit` defaults to A4 when used for text layout).
    pub fn dimensions_pt(&self) -> (f32, f32) {
        match self {
            Self::Fit | Self::A4 => (595.28, 841.89),
            Self::Letter => (612.0, 792.0),
        }
    }
}

/// Options for [`pdf_to_images`].
#[derive(Debug, Clone)]
pub struct PdfToImagesOptions {
    /// Loading and decryption options for input PDF.
    pub load: LoadOptions,
    /// Output image format (`png` or `jpg`).
    pub format: ImageFormat,
    /// Target rendering resolution in dots per inch (`18..=1200`, default `150`).
    pub dpi: u32,
    /// JPEG quality (`1..=100`, default `85`; ignored for PNG).
    pub quality: u8,
    /// Optional 1-based page selection (`None` exports all pages).
    pub pages: Option<String>,
    /// Target output directory (defaults to `{stem}_images/` next to the source PDF).
    pub out_dir: Option<PathBuf>,
    /// Overwrite existing image files if `true`.
    pub force: bool,
    /// Compute planned outputs without writing files if `true`.
    pub dry_run: bool,
}

impl Default for PdfToImagesOptions {
    fn default() -> Self {
        Self {
            load: LoadOptions::default(),
            format: ImageFormat::Png,
            dpi: 150,
            quality: 85,
            pages: None,
            out_dir: None,
            force: false,
            dry_run: false,
        }
    }
}

/// Metadata for a single exported page image in [`PdfToImagesReport`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConvertedImagePage {
    /// 1-based page number in the source PDF.
    pub page: u32,
    /// Output image file path.
    pub path: PathBuf,
    /// Image width in pixels.
    pub width_px: u32,
    /// Image height in pixels.
    pub height_px: u32,
}

/// Report returned by [`pdf_to_images`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PdfToImagesReport {
    /// Source PDF file path.
    pub input: PathBuf,
    /// Output directory where images were written.
    pub output_dir: PathBuf,
    /// Output format (`"png"` or `"jpg"`).
    pub format: &'static str,
    /// Resolution in DPI.
    pub dpi: u32,
    /// Total pages in the source PDF.
    pub total_pages: u32,
    /// Exported image files.
    pub outputs: Vec<ConvertedImagePage>,
    /// Indicates whether `--dry-run` was active.
    pub dry_run: bool,
}

/// Options for [`images_to_pdf`].
#[derive(Debug, Clone)]
pub struct ImagesToPdfOptions {
    /// Target page size (`fit`, `a4`, or `letter`).
    pub page_size: ConvertPageSize,
    /// Page margin in PDF points (`0.0..=200.0`, default `0.0`).
    pub margin_pt: f32,
    /// Target output PDF file path.
    pub out: PathBuf,
    /// Overwrite existing output file if `true`.
    pub force: bool,
    /// Compute planned output without writing the file if `true`.
    pub dry_run: bool,
}

/// Report returned by [`images_to_pdf`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImagesToPdfReport {
    /// Input image file paths.
    pub inputs: Vec<PathBuf>,
    /// Output PDF file path.
    pub output: PathBuf,
    /// Page size mode (`"fit"`, `"a4"`, `"letter"`).
    pub page_size: &'static str,
    /// Margin in PDF points.
    pub margin_pt: f32,
    /// Total pages created in the output PDF.
    pub pages: u32,
    /// Indicates whether `--dry-run` was active.
    pub dry_run: bool,
}

/// Options for [`pdf_to_markdown`].
#[derive(Debug, Clone, Default)]
pub struct PdfToMarkdownOptions {
    /// Loading and decryption options for input PDF.
    pub load: LoadOptions,
    /// Optional 1-based page selection (`None` extracts all pages).
    pub pages: Option<String>,
    /// Target `.md` file path (defaults to `{stem}.md` next to the source PDF).
    pub out: Option<PathBuf>,
    /// Overwrite existing output file if `true`.
    pub force: bool,
    /// Compute extraction without writing the output file if `true`.
    pub dry_run: bool,
}

/// Report returned by [`pdf_to_markdown`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PdfToMarkdownReport {
    /// Input PDF file path.
    pub input: PathBuf,
    /// Output Markdown file path.
    pub output: PathBuf,
    /// Total pages in the source PDF.
    pub total_pages: u32,
    /// Number of pages extracted.
    pub extracted_pages: u32,
    /// Total characters in the generated Markdown.
    pub char_count: usize,
    /// Total lines in the generated Markdown.
    pub line_count: usize,
    /// Indicates whether `--dry-run` was active.
    pub dry_run: bool,
}

/// Options for [`markdown_to_pdf`].
#[derive(Debug, Clone)]
pub struct MarkdownToPdfOptions {
    /// Target page size (`a4` or `letter`; `fit` is treated as `a4`).
    pub page_size: ConvertPageSize,
    /// Page margin in PDF points (`18.0..=144.0`, default `54.0`).
    pub margin_pt: f32,
    /// Target `.pdf` file path (defaults to `{stem}.pdf` next to the source `.md`).
    pub out: Option<PathBuf>,
    /// Overwrite existing output file if `true`.
    pub force: bool,
    /// Compute layout without writing the output file if `true`.
    pub dry_run: bool,
}

impl Default for MarkdownToPdfOptions {
    fn default() -> Self {
        Self {
            page_size: ConvertPageSize::A4,
            margin_pt: 54.0,
            out: None,
            force: false,
            dry_run: false,
        }
    }
}

/// Report returned by [`markdown_to_pdf`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MarkdownToPdfReport {
    /// Input Markdown file path.
    pub input: PathBuf,
    /// Output PDF file path.
    pub output: PathBuf,
    /// Page size (`"a4"` or `"letter"`).
    pub page_size: &'static str,
    /// Margin in PDF points.
    pub margin_pt: f32,
    /// Total pages generated in the output PDF.
    pub pages: u32,
    /// Number of headings converted into PDF bookmarks/outlines.
    pub headings_count: u32,
    /// Indicates whether `--dry-run` was active.
    pub dry_run: bool,
}

/// Unified report enum covering all conversion directions (`to-images`, `from-images`, `to-markdown`, `from-markdown`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum ConvertReport {
    /// Report from [`pdf_to_images`].
    ToImages(PdfToImagesReport),
    /// Report from [`images_to_pdf`].
    FromImages(ImagesToPdfReport),
    /// Report from [`pdf_to_markdown`].
    ToMarkdown(PdfToMarkdownReport),
    /// Report from [`markdown_to_pdf`].
    FromMarkdown(MarkdownToPdfReport),
}

/// Exports pages of a PDF (`src`) as `PNG` or `JPG` images into a directory.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use sheaf_core::{NoProgress, PdfToImagesOptions, pdf_to_images};
///
/// let report = pdf_to_images(
///     Path::new("report.pdf"),
///     &PdfToImagesOptions::default(),
///     &NoProgress,
/// ).unwrap();
/// println!("Exported {} images", report.outputs.len());
/// ```
pub fn pdf_to_images(
    src: &Path,
    opts: &PdfToImagesOptions,
    p: &dyn Progress,
) -> Result<PdfToImagesReport, SheafError> {
    if opts.dpi < 18 || opts.dpi > 1200 {
        return Err(SheafError::InvalidSelection(format!(
            "DPI must be between 18 and 1200 (got {})",
            opts.dpi
        )));
    }
    if opts.quality == 0 || opts.quality > 100 {
        return Err(SheafError::InvalidSelection(format!(
            "JPEG quality must be between 1 and 100 (got {})",
            opts.quality
        )));
    }
    if !src.exists() {
        return Err(SheafError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found: {}", src.display()),
        )));
    }

    let out_dir = match &opts.out_dir {
        Some(dir) => dir.clone(),
        None => {
            let parent = src.parent().unwrap_or_else(|| Path::new("."));
            let stem = sanitize_stem(src);
            parent.join(format!("{stem}_images"))
        }
    };
    ensure_not_overwriting_inputs(&[src.to_path_buf()], &out_dir)?;

    let pdfium = get_pdfium()?;
    let doc = load_pdfium_doc(pdfium, src, &opts.load)?;
    let total_pages = u32::try_from(doc.pages().len()).unwrap_or(0);
    if total_pages == 0 {
        return Err(SheafError::InvalidPdf(format!(
            "{}: Document contains 0 pages",
            src.display()
        )));
    }

    let page_numbers: Vec<u32> = match &opts.pages {
        Some(spec) => PageSelection::parse(spec, total_pages)?.to_page_numbers(),
        None => (1..=total_pages).collect(),
    };

    let stem = sanitize_stem(src);
    let pad_width = total_pages.to_string().len().max(3);
    let ext = opts.format.extension();

    let mut planned = Vec::with_capacity(page_numbers.len());
    for &page_num in &page_numbers {
        let filename = format!("{stem}_p{page_num:0pad_width$}.{ext}");
        let target = out_dir.join(filename);
        ensure_not_overwriting_inputs(&[src.to_path_buf()], &target)?;
        if !opts.dry_run {
            check_output_path(&target, opts.force)?;
        }
        planned.push((page_num, target));
    }

    let total_steps = planned.len() as u64;
    let mut outputs = Vec::with_capacity(planned.len());

    for (idx, (page_num, target_path)) in planned.into_iter().enumerate() {
        if p.is_cancelled() {
            return Err(SheafError::Cancelled);
        }
        p.report(idx as u64, total_steps);

        let page_index = i32::try_from(page_num - 1).map_err(|_| SheafError::PageOutOfRange {
            page: page_num,
            total: total_pages,
        })?;
        let page = doc
            .pages()
            .get(page_index)
            .map_err(|_| SheafError::PageOutOfRange {
                page: page_num,
                total: total_pages,
            })?;

        let width_pt = page.width().value;
        let height_pt = page.height().value;
        let target_w = ((width_pt * (opts.dpi as f32) / 72.0).round() as i32).max(1);
        let target_h = ((height_pt * (opts.dpi as f32) / 72.0).round() as i32).max(1);

        if opts.dry_run {
            outputs.push(ConvertedImagePage {
                page: page_num,
                path: target_path,
                width_px: target_w as u32,
                height_px: target_h as u32,
            });
            continue;
        }

        let config = PdfRenderConfig::new()
            .set_target_width(target_w)
            .set_maximum_height(target_h * 2)
            .render_form_data(true);

        let bitmap = page.render_with_config(&config).map_err(|e| {
            SheafError::InvalidPdf(format!(
                "{}: PDFium render error on page {page_num}: {e}",
                src.display()
            ))
        })?;

        let dyn_img = bitmap.as_image().map_err(|e| {
            SheafError::InvalidPdf(format!("Failed to convert rendered bitmap to image: {e}"))
        })?;
        let width_px = dyn_img.width();
        let height_px = dyn_img.height();

        let encoded = encode_dynamic_image(&dyn_img, opts.format, opts.quality)?;
        write_atomic(&target_path, opts.force, |file| {
            let mut writer = BufWriter::new(file);
            writer.write_all(&encoded)?;
            writer.flush()?;
            Ok(())
        })?;

        outputs.push(ConvertedImagePage {
            page: page_num,
            path: target_path,
            width_px,
            height_px,
        });
    }

    p.report(total_steps, total_steps);

    Ok(PdfToImagesReport {
        input: src.to_path_buf(),
        output_dir: out_dir,
        format: opts.format.extension(),
        dpi: opts.dpi,
        total_pages,
        outputs,
        dry_run: opts.dry_run,
    })
}

/// Combines one or more image files (`PNG`, `JPG`/`JPEG`) into a single multi-page PDF.
///
/// # Examples
///
/// ```no_run
/// use std::path::PathBuf;
/// use sheaf_core::{ConvertPageSize, ImagesToPdfOptions, NoProgress, images_to_pdf};
///
/// let opts = ImagesToPdfOptions {
///     page_size: ConvertPageSize::A4,
///     margin_pt: 36.0,
///     out: PathBuf::from("scan.pdf"),
///     force: true,
///     dry_run: false,
/// };
/// let report = images_to_pdf(&["page1.png", "page2.jpg"], &opts, &NoProgress).unwrap();
/// assert_eq!(report.pages, 2);
/// ```
pub fn images_to_pdf<P: AsRef<Path>>(
    inputs: &[P],
    opts: &ImagesToPdfOptions,
    p: &dyn Progress,
) -> Result<ImagesToPdfReport, SheafError> {
    if inputs.is_empty() {
        return Err(SheafError::EmptySelection);
    }
    if !(0.0..=200.0).contains(&opts.margin_pt) {
        return Err(SheafError::InvalidSelection(format!(
            "Margin must be between 0 and 200 pt (got {})",
            opts.margin_pt
        )));
    }

    let input_paths: Vec<PathBuf> = inputs.iter().map(|i| i.as_ref().to_path_buf()).collect();
    ensure_not_overwriting_inputs(&input_paths, &opts.out)?;
    if !opts.dry_run {
        check_output_path(&opts.out, opts.force)?;
    }

    let total_steps = input_paths.len() as u64;
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_ids = Vec::with_capacity(input_paths.len());

    for (idx, img_path) in input_paths.iter().enumerate() {
        if p.is_cancelled() {
            return Err(SheafError::Cancelled);
        }
        p.report(idx as u64, total_steps);

        if !img_path.exists() {
            return Err(SheafError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Image file not found: {}", img_path.display()),
            )));
        }

        let reader = ImageReader::open(img_path)?
            .with_guessed_format()
            .map_err(SheafError::Io)?;
        let dyn_img = reader.decode().map_err(|e| {
            SheafError::InvalidSelection(format!(
                "Failed to decode image '{}': {e}",
                img_path.display()
            ))
        })?;

        let img_w = dyn_img.width().max(1);
        let img_h = dyn_img.height().max(1);

        if opts.dry_run {
            continue;
        }

        let rgb = dyn_img.to_rgb8();
        let mut jpeg_bytes = Vec::new();
        let encoder = JpegEncoder::new_with_quality(&mut jpeg_bytes, 92);
        encoder
            .write_image(rgb.as_raw(), img_w, img_h, ExtendedColorType::Rgb8)
            .map_err(|e| {
                SheafError::Io(std::io::Error::other(format!(
                    "Failed to encode image stream: {e}"
                )))
            })?;

        let (page_w, page_h, draw_x, draw_y, draw_w, draw_h) =
            compute_image_placement(img_w as f32, img_h as f32, opts.page_size, opts.margin_pt);

        let img_id = doc.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => i64::from(img_w),
                "Height" => i64::from(img_h),
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
                "Filter" => "DCTDecode",
            },
            jpeg_bytes,
        ));

        let content = Content {
            operations: vec![
                Operation::new("q", vec![]),
                Operation::new(
                    "cm",
                    vec![
                        Object::Real(draw_w),
                        0.into(),
                        0.into(),
                        Object::Real(draw_h),
                        Object::Real(draw_x),
                        Object::Real(draw_y),
                    ],
                ),
                Operation::new("Do", vec!["Im1".into()]),
                Operation::new("Q", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(
            dictionary! {},
            content.encode().map_err(|e| {
                SheafError::InvalidPdf(format!("Failed to encode page content stream: {e}"))
            })?,
        ));
        let res_id = doc.add_object(dictionary! {
            "XObject" => dictionary! { "Im1" => img_id },
        });
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), Object::Real(page_w), Object::Real(page_h)],
            "Resources" => res_id,
            "Contents" => content_id,
        });
        page_ids.push(Object::Reference(page_id));
    }

    let pages_count = input_paths.len() as u32;
    if !opts.dry_run {
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => page_ids,
                "Count" => i64::from(pages_count),
            }),
        );
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);
        doc.compress();

        write_atomic(&opts.out, opts.force, |file| {
            let mut writer = BufWriter::new(file);
            doc.save_to(&mut writer)
                .map_err(|e| std::io::Error::other(format!("Failed to write PDF: {e}")))?;
            writer.flush()?;
            Ok(())
        })?;
    }

    p.report(total_steps, total_steps);

    Ok(ImagesToPdfReport {
        inputs: input_paths,
        output: opts.out.clone(),
        page_size: opts.page_size.as_str(),
        margin_pt: opts.margin_pt,
        pages: pages_count,
        dry_run: opts.dry_run,
    })
}

/// Extracts structured Markdown (`.md`) from a PDF file (`src`).
///
/// Preserves document title/headings (via bookmarks and font-size heuristics), bullet lists,
/// and paragraphs across all or selected pages.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use sheaf_core::{NoProgress, PdfToMarkdownOptions, pdf_to_markdown};
///
/// let report = pdf_to_markdown(
///     Path::new("report.pdf"),
///     &PdfToMarkdownOptions::default(),
///     &NoProgress,
/// ).unwrap();
/// println!("Extracted {} characters", report.char_count);
/// ```
pub fn pdf_to_markdown(
    src: &Path,
    opts: &PdfToMarkdownOptions,
    p: &dyn Progress,
) -> Result<PdfToMarkdownReport, SheafError> {
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
            parent.join(format!("{stem}.md"))
        }
    };
    ensure_not_overwriting_inputs(&[src.to_path_buf()], &out_path)?;
    if !opts.dry_run {
        check_output_path(&out_path, opts.force)?;
    }

    let raw_bytes = fs::read(src)?;
    let loaded = load_pdf_from_bytes(&raw_bytes, src, &opts.load)?;
    let doc = loaded.doc;

    let pages_map = doc.get_pages();
    let total_pages = pages_map.len() as u32;
    if total_pages == 0 {
        return Err(SheafError::InvalidPdf(format!(
            "{}: Document contains 0 pages",
            src.display()
        )));
    }

    let page_numbers: Vec<u32> = match &opts.pages {
        Some(spec) => PageSelection::parse(spec, total_pages)?.to_page_numbers(),
        None => (1..=total_pages).collect(),
    };

    let bookmarks_by_page = collect_bookmarks_by_page(&doc, &pages_map);
    let total_steps = page_numbers.len() as u64;
    let mut md_sections: Vec<String> = Vec::with_capacity(page_numbers.len());

    for (idx, &page_num) in page_numbers.iter().enumerate() {
        if p.is_cancelled() {
            return Err(SheafError::Cancelled);
        }
        p.report(idx as u64, total_steps);

        let mut page_lines: Vec<String> = Vec::new();

        // Include any PDF outline/bookmark headings targeting this page
        if let Some(bm_list) = bookmarks_by_page.get(&page_num) {
            for (level, title) in bm_list {
                let hashes = "#".repeat((*level as usize).clamp(1, 6));
                page_lines.push(format!("{hashes} {}", title.trim()));
                page_lines.push(String::new());
            }
        }

        let extracted = extract_structured_page_markdown(&doc, &pages_map, page_num);
        if !extracted.trim().is_empty() {
            // Avoid duplicating a heading that was already emitted from bookmarks
            for line in extracted.lines() {
                if let Some(bm_list) = bookmarks_by_page.get(&page_num) {
                    let clean_line = line.trim().trim_start_matches('#').trim();
                    if bm_list.iter().any(|(_, t)| t.trim() == clean_line)
                        && line.trim_start().starts_with('#')
                    {
                        continue;
                    }
                }
                page_lines.push(line.to_string());
            }
        }

        let section = collapse_blank_lines(&page_lines.join("\n"));
        if !section.is_empty() {
            md_sections.push(section);
        }
    }

    let mut markdown = md_sections.join("\n\n---\n\n");
    if !markdown.ends_with('\n') && !markdown.is_empty() {
        markdown.push('\n');
    }

    let char_count = markdown.chars().count();
    let line_count = markdown.lines().count();

    if !opts.dry_run {
        write_atomic(&out_path, opts.force, |file| {
            let mut writer = BufWriter::new(file);
            writer.write_all(markdown.as_bytes())?;
            writer.flush()?;
            Ok(())
        })?;
    }

    p.report(total_steps, total_steps);

    Ok(PdfToMarkdownReport {
        input: src.to_path_buf(),
        output: out_path,
        total_pages,
        extracted_pages: page_numbers.len() as u32,
        char_count,
        line_count,
        dry_run: opts.dry_run,
    })
}

/// Renders a Markdown (`.md`) file (`src`) into a clean, multi-page vector PDF with selectable text
/// and automatic PDF bookmarks for headings.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use sheaf_core::{MarkdownToPdfOptions, NoProgress, markdown_to_pdf};
///
/// let report = markdown_to_pdf(
///     Path::new("notes.md"),
///     &MarkdownToPdfOptions::default(),
///     &NoProgress,
/// ).unwrap();
/// println!("Generated {} PDF pages", report.pages);
/// ```
pub fn markdown_to_pdf(
    src: &Path,
    opts: &MarkdownToPdfOptions,
    p: &dyn Progress,
) -> Result<MarkdownToPdfReport, SheafError> {
    if !(18.0..=144.0).contains(&opts.margin_pt) {
        return Err(SheafError::InvalidSelection(format!(
            "Margin must be between 18 and 144 pt (got {})",
            opts.margin_pt
        )));
    }
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
            parent.join(format!("{stem}.pdf"))
        }
    };
    ensure_not_overwriting_inputs(&[src.to_path_buf()], &out_path)?;
    if !opts.dry_run {
        check_output_path(&out_path, opts.force)?;
    }

    let md_text = fs::read_to_string(src)?;
    let blocks = parse_markdown_blocks(&md_text);
    if blocks.is_empty() {
        return Err(SheafError::EmptySelection);
    }

    let effective_page_size = match opts.page_size {
        ConvertPageSize::Fit => ConvertPageSize::A4,
        other => other,
    };
    let (page_w, page_h) = effective_page_size.dimensions_pt();
    let margin = opts.margin_pt;
    let content_w = (page_w - 2.0 * margin).max(100.0);

    let (pages_ops, headings) =
        layout_markdown_pages(&blocks, page_w, page_h, margin, content_w, p)?;

    let pages_count = pages_ops.len().max(1) as u32;
    let headings_count = headings.len() as u32;

    if !opts.dry_run {
        let mut doc = build_markdown_pdf_document(pages_ops, &headings, page_w, page_h, &out_path)?;
        doc.compress();
        write_atomic(&out_path, opts.force, |file| {
            let mut writer = BufWriter::new(file);
            doc.save_to(&mut writer)
                .map_err(|e| std::io::Error::other(format!("Failed to save PDF: {e}")))?;
            writer.flush()?;
            Ok(())
        })?;
    }

    p.report(100, 100);

    Ok(MarkdownToPdfReport {
        input: src.to_path_buf(),
        output: out_path,
        page_size: effective_page_size.as_str(),
        margin_pt: margin,
        pages: pages_count,
        headings_count,
        dry_run: opts.dry_run,
    })
}

fn load_pdfium_doc<'a>(
    pdfium: &'a Pdfium,
    src: &Path,
    load_opts: &LoadOptions,
) -> Result<PdfDocument<'a>, SheafError> {
    let pw_str = load_opts.password.as_ref().map(|s| s.expose_secret());
    match pdfium.load_pdf_from_file(src, pw_str) {
        Ok(doc) => Ok(doc),
        Err(err) => {
            load_pdf(src, load_opts)?;
            let err_str = err.to_string();
            if err_str.to_ascii_lowercase().contains("password")
                || err_str.to_ascii_lowercase().contains("encrypt")
            {
                if pw_str.is_some() {
                    return Err(SheafError::WrongPassword);
                } else {
                    return Err(SheafError::Encrypted);
                }
            }
            Err(SheafError::InvalidPdf(format!(
                "{}: PDFium failed to load document: {err_str}",
                src.display()
            )))
        }
    }
}

fn encode_dynamic_image(
    img: &DynamicImage,
    format: ImageFormat,
    quality: u8,
) -> Result<Vec<u8>, SheafError> {
    let mut buf = Cursor::new(Vec::new());
    match format {
        ImageFormat::Png => {
            let rgba = img.to_rgba8();
            let encoder = PngEncoder::new_with_quality(
                &mut buf,
                CompressionType::Fast,
                PngFilterType::Adaptive,
            );
            encoder
                .write_image(
                    rgba.as_raw(),
                    rgba.width(),
                    rgba.height(),
                    ExtendedColorType::Rgba8,
                )
                .map_err(|e| {
                    SheafError::Io(std::io::Error::other(format!("PNG encode error: {e}")))
                })?;
        }
        ImageFormat::Jpg => {
            let rgb = img.to_rgb8();
            let mut encoder = JpegEncoder::new_with_quality(&mut buf, quality);
            encoder
                .encode(
                    rgb.as_raw(),
                    rgb.width(),
                    rgb.height(),
                    ColorType::Rgb8.into(),
                )
                .map_err(|e| {
                    SheafError::Io(std::io::Error::other(format!("JPEG encode error: {e}")))
                })?;
        }
    }
    Ok(buf.into_inner())
}

fn compute_image_placement(
    img_w: f32,
    img_h: f32,
    page_size: ConvertPageSize,
    margin_pt: f32,
) -> (f32, f32, f32, f32, f32, f32) {
    match page_size {
        ConvertPageSize::Fit => {
            let page_w = img_w + 2.0 * margin_pt;
            let page_h = img_h + 2.0 * margin_pt;
            (page_w, page_h, margin_pt, margin_pt, img_w, img_h)
        }
        ConvertPageSize::A4 | ConvertPageSize::Letter => {
            let (page_w, page_h) = page_size.dimensions_pt();
            let avail_w = (page_w - 2.0 * margin_pt).max(1.0);
            let avail_h = (page_h - 2.0 * margin_pt).max(1.0);
            let scale = (avail_w / img_w).min(avail_h / img_h);
            let draw_w = img_w * scale;
            let draw_h = img_h * scale;
            let draw_x = (page_w - draw_w) * 0.5;
            let draw_y = (page_h - draw_h) * 0.5;
            (page_w, page_h, draw_x, draw_y, draw_w, draw_h)
        }
    }
}

fn collect_bookmarks_by_page(
    doc: &Document,
    pages_map: &BTreeMap<u32, ObjectId>,
) -> BTreeMap<u32, Vec<(u32, String)>> {
    let mut id_to_page = BTreeMap::new();
    for (&num, &id) in pages_map {
        id_to_page.insert(id, num);
    }

    let mut result: BTreeMap<u32, Vec<(u32, String)>> = BTreeMap::new();
    let Ok(catalog) = doc.catalog() else {
        return result;
    };
    let Ok(outlines_ref) = catalog.get(b"Outlines").and_then(Object::as_reference) else {
        return result;
    };
    let Ok(Object::Dictionary(outlines_dict)) = doc.get_object(outlines_ref) else {
        return result;
    };
    let Ok(first_id) = outlines_dict.get(b"First").and_then(Object::as_reference) else {
        return result;
    };

    traverse_outline_items(doc, first_id, 1, &id_to_page, &mut result, 0);
    result
}

fn traverse_outline_items(
    doc: &Document,
    first_id: ObjectId,
    level: u32,
    id_to_page: &BTreeMap<ObjectId, u32>,
    out: &mut BTreeMap<u32, Vec<(u32, String)>>,
    depth_guard: usize,
) {
    if depth_guard > 64 {
        return;
    }
    let mut current = Some(first_id);
    let mut visited = 0usize;
    while let Some(item_id) = current {
        visited += 1;
        if visited > 2000 {
            break;
        }
        let Ok(Object::Dictionary(dict)) = doc.get_object(item_id) else {
            break;
        };

        if let Some(title) = dict.get(b"Title").ok().and_then(decode_pdf_text_object)
            && let Some(page_num) = resolve_outline_page(doc, dict, id_to_page)
        {
            out.entry(page_num).or_default().push((level, title));
        }

        if let Ok(child_first) = dict.get(b"First").and_then(Object::as_reference) {
            traverse_outline_items(
                doc,
                child_first,
                level + 1,
                id_to_page,
                out,
                depth_guard + 1,
            );
        }

        current = dict.get(b"Next").and_then(Object::as_reference).ok();
    }
}

fn resolve_outline_page(
    doc: &Document,
    dict: &Dictionary,
    id_to_page: &BTreeMap<ObjectId, u32>,
) -> Option<u32> {
    if let Ok(dest) = dict.get(b"Dest")
        && let Some(p) = resolve_dest_array(doc, dest, id_to_page)
    {
        return Some(p);
    }
    if let Ok(action_obj) = dict.get(b"A") {
        let action_dict = match action_obj {
            Object::Reference(id) => doc.get_object(*id).ok().and_then(|o| o.as_dict().ok()),
            Object::Dictionary(d) => Some(d),
            _ => None,
        }?;
        if let Ok(dest) = action_dict.get(b"D") {
            return resolve_dest_array(doc, dest, id_to_page);
        }
    }
    None
}

fn resolve_dest_array(
    doc: &Document,
    dest: &Object,
    id_to_page: &BTreeMap<ObjectId, u32>,
) -> Option<u32> {
    let arr = match dest {
        Object::Array(a) => Some(a.as_slice()),
        Object::Reference(id) => doc
            .get_object(*id)
            .ok()
            .and_then(|o| o.as_array().ok())
            .map(Vec::as_slice),
        _ => None,
    }?;
    let first = arr.first()?;
    if let Object::Reference(page_id) = first {
        id_to_page.get(page_id).copied()
    } else {
        None
    }
}

fn decode_pdf_text_object(obj: &Object) -> Option<String> {
    match obj {
        Object::String(bytes, _) => Some(decode_win_ansi(bytes)),
        _ => None,
    }
}

fn decode_win_ansi(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFE, 0xFF]) && bytes.len() >= 4 {
        let u16s: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&c| u16::from_be_bytes(c))
            .collect();
        if let Ok(s) = String::from_utf16(&u16s) {
            return s;
        }
    }
    bytes
        .iter()
        .map(|&b| match b {
            0x95 => '•',
            0x96 => '–',
            0x97 => '—',
            0x85 => '…',
            other => char::from(other),
        })
        .collect()
}

fn extract_structured_page_markdown(
    doc: &Document,
    pages_map: &BTreeMap<u32, ObjectId>,
    page_num: u32,
) -> String {
    let Some(&page_id) = pages_map.get(&page_num) else {
        return String::new();
    };

    // Inspect content stream operations for font sizes and text lines
    let raw_content = doc.get_page_content(page_id);
    if !raw_content.is_empty()
        && let Ok(content) = Content::decode(&raw_content)
    {
        let lines = extract_lines_from_operations(&content.operations);
        if !lines.is_empty() {
            return lines.join("\n");
        }
    }

    // Fallback to lopdf text extraction
    if let Ok(text) = doc.extract_text(&[page_num])
        && !text.trim().is_empty()
    {
        return text.trim().to_string();
    }

    String::new()
}

fn extract_lines_from_operations(ops: &[Operation]) -> Vec<String> {
    let mut font_size: f32 = 11.0;
    let mut current_font = String::new();
    let mut current_line = String::new();
    let mut lines: Vec<(f32, String, String)> = Vec::new();
    let mut in_code_block = false;

    for op in ops {
        match op.operator.as_str() {
            "BT" => {
                if !current_line.trim().is_empty() {
                    lines.push((
                        font_size,
                        current_font.clone(),
                        std::mem::take(&mut current_line),
                    ));
                }
            }
            "ET" => {
                if !current_line.trim().is_empty() {
                    lines.push((
                        font_size,
                        current_font.clone(),
                        std::mem::take(&mut current_line),
                    ));
                }
            }
            "Tf" => {
                if op.operands.len() >= 2 {
                    if let Ok(name) = op.operands[0].as_name() {
                        current_font = String::from_utf8_lossy(name).to_string();
                    }
                    if let Some(sz) = obj_to_f32(&op.operands[1]) {
                        font_size = sz.abs();
                    }
                }
            }
            "Td" | "TD" => {
                if op.operands.len() >= 2
                    && let Some(dy) = obj_to_f32(&op.operands[1])
                    && dy.abs() > 1.0
                    && !current_line.trim().is_empty()
                {
                    lines.push((
                        font_size,
                        current_font.clone(),
                        std::mem::take(&mut current_line),
                    ));
                }
            }
            "T*" => {
                if !current_line.trim().is_empty() {
                    lines.push((
                        font_size,
                        current_font.clone(),
                        std::mem::take(&mut current_line),
                    ));
                }
            }
            "Tj" | "'" | "\"" => {
                if let Some(last) = op.operands.last()
                    && let Some(s) = decode_pdf_text_object(last)
                {
                    current_line.push_str(&s);
                }
            }
            "TJ" => {
                if let Some(Object::Array(arr)) = op.operands.first() {
                    for item in arr {
                        if let Some(s) = decode_pdf_text_object(item) {
                            current_line.push_str(&s);
                        } else if let Some(kern) = obj_to_f32(item)
                            && kern < -120.0
                            && !current_line.ends_with(' ')
                        {
                            current_line.push(' ');
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if !current_line.trim().is_empty() {
        lines.push((font_size, current_font, current_line));
    }

    let mut md_out: Vec<String> = Vec::new();
    let mut pending_marker: Option<String> = None;

    for (sz, font, text) in lines {
        let trimmed = text.trim_end();
        if trimmed.trim().is_empty() {
            continue;
        }
        let is_mono = font == "F4" || font.to_ascii_lowercase().contains("courier");
        if is_mono && sz <= 10.5 {
            if !in_code_block {
                md_out.push("```".to_string());
                in_code_block = true;
            }
            md_out.push(trimmed.to_string());
            continue;
        } else if in_code_block {
            md_out.push("```".to_string());
            md_out.push(String::new());
            in_code_block = false;
        }

        let clean = trimmed.trim();
        if clean == "•" {
            pending_marker = Some("-".to_string());
            continue;
        }
        if clean.ends_with('.')
            && clean.len() <= 4
            && clean[..clean.len() - 1].chars().all(|c| c.is_ascii_digit())
        {
            pending_marker = Some(clean.to_string());
            continue;
        }

        if let Some(marker) = pending_marker.take() {
            md_out.push(format!("{marker} {clean}"));
            continue;
        }

        if sz >= 20.0 {
            md_out.push(format!("# {clean}"));
            md_out.push(String::new());
        } else if sz >= 16.5 {
            md_out.push(format!("## {clean}"));
            md_out.push(String::new());
        } else if sz >= 13.5 {
            md_out.push(format!("### {clean}"));
            md_out.push(String::new());
        } else if let Some(rest) = clean.strip_prefix("• ") {
            md_out.push(format!("- {rest}"));
        } else {
            md_out.push(clean.to_string());
        }
    }
    if in_code_block {
        md_out.push("```".to_string());
    }

    md_out
}

fn obj_to_f32(obj: &Object) -> Option<f32> {
    match obj {
        Object::Real(v) => Some(*v),
        Object::Integer(v) => Some(*v as f32),
        _ => None,
    }
}

fn collapse_blank_lines(input: &str) -> String {
    let mut out = Vec::new();
    let mut prev_blank = true;
    for line in input.lines() {
        let is_blank = line.trim().is_empty();
        if is_blank {
            if !prev_blank {
                out.push("");
            }
            prev_blank = true;
        } else {
            out.push(line.trim_end());
            prev_blank = false;
        }
    }
    while out.last() == Some(&"") {
        out.pop();
    }
    out.join("\n")
}

#[derive(Debug, Clone)]
enum MdBlock {
    Heading { level: u8, text: String },
    Paragraph { text: String },
    ListItem { marker: String, text: String },
    Blockquote { text: String },
    CodeBlock { lines: Vec<String> },
    HorizontalRule,
}

#[derive(Debug, Clone)]
struct HeadingEntry {
    level: u8,
    title: String,
    page_index: usize,
}

fn parse_markdown_blocks(md: &str) -> Vec<MdBlock> {
    let mut blocks = Vec::new();
    let mut paragraph_buf = Vec::new();
    let mut in_code_block = false;
    let mut code_lines = Vec::new();

    let flush_paragraph = |buf: &mut Vec<String>, out: &mut Vec<MdBlock>| {
        if !buf.is_empty() {
            let joined = buf.join(" ");
            let cleaned = strip_inline_markdown(&joined);
            if !cleaned.trim().is_empty() {
                out.push(MdBlock::Paragraph { text: cleaned });
            }
            buf.clear();
        }
    };

    for raw_line in md.lines() {
        let trimmed_start = raw_line.trim_start();
        if trimmed_start.starts_with("```") {
            flush_paragraph(&mut paragraph_buf, &mut blocks);
            if in_code_block {
                blocks.push(MdBlock::CodeBlock {
                    lines: std::mem::take(&mut code_lines),
                });
                in_code_block = false;
            } else {
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            code_lines.push(raw_line.to_string());
            continue;
        }

        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            flush_paragraph(&mut paragraph_buf, &mut blocks);
            continue;
        }

        // Horizontal rule
        if (trimmed.len() >= 3
            && (trimmed.chars().all(|c| c == '-' || c == ' ')
                || trimmed.chars().all(|c| c == '*' || c == ' ')
                || trimmed.chars().all(|c| c == '_' || c == ' ')))
            && trimmed
                .chars()
                .filter(|&c| c == '-' || c == '*' || c == '_')
                .count()
                >= 3
            && !trimmed.starts_with("- ")
            && !trimmed.starts_with("* ")
        {
            flush_paragraph(&mut paragraph_buf, &mut blocks);
            blocks.push(MdBlock::HorizontalRule);
            continue;
        }

        // Heading (# .. ######)
        let hash_count = trimmed.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&hash_count) && trimmed[hash_count..].starts_with(' ') {
            flush_paragraph(&mut paragraph_buf, &mut blocks);
            let title = strip_inline_markdown(trimmed[hash_count..].trim());
            if !title.is_empty() {
                blocks.push(MdBlock::Heading {
                    level: hash_count as u8,
                    text: title,
                });
            }
            continue;
        }

        // Blockquote (> ...)
        if let Some(rest) = trimmed.strip_prefix('>') {
            flush_paragraph(&mut paragraph_buf, &mut blocks);
            let text = strip_inline_markdown(rest.trim());
            if !text.is_empty() {
                blocks.push(MdBlock::Blockquote { text });
            }
            continue;
        }

        // Unordered list (- , * , + )
        if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("+ "))
        {
            flush_paragraph(&mut paragraph_buf, &mut blocks);
            blocks.push(MdBlock::ListItem {
                marker: "•".to_string(),
                text: strip_inline_markdown(rest.trim()),
            });
            continue;
        }

        // Ordered list (1. , 2. , ...)
        if let Some((num_part, rest)) = parse_ordered_list_line(trimmed) {
            flush_paragraph(&mut paragraph_buf, &mut blocks);
            blocks.push(MdBlock::ListItem {
                marker: format!("{num_part}."),
                text: strip_inline_markdown(rest.trim()),
            });
            continue;
        }

        paragraph_buf.push(trimmed.to_string());
    }

    if in_code_block && !code_lines.is_empty() {
        blocks.push(MdBlock::CodeBlock { lines: code_lines });
    }
    flush_paragraph(&mut paragraph_buf, &mut blocks);

    blocks
}

fn parse_ordered_list_line(line: &str) -> Option<(&str, &str)> {
    let dot_idx = line.find(". ")?;
    let prefix = &line[..dot_idx];
    if !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_digit()) {
        Some((prefix, &line[dot_idx + 2..]))
    } else {
        None
    }
}

fn strip_inline_markdown(input: &str) -> String {
    // Convert [label](url) -> label
    let mut s = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '['
            && let Some(close_bracket) = chars[i + 1..].iter().position(|&c| c == ']')
        {
            let close_idx = i + 1 + close_bracket;
            if close_idx + 1 < chars.len()
                && chars[close_idx + 1] == '('
                && let Some(close_paren) = chars[close_idx + 2..].iter().position(|&c| c == ')')
            {
                let label: String = chars[i + 1..close_idx].iter().collect();
                s.push_str(&label);
                i = close_idx + 2 + close_paren + 1;
                continue;
            }
        }
        s.push(chars[i]);
        i += 1;
    }

    s.replace("**", "")
        .replace("__", "")
        .replace('`', "")
        .trim()
        .to_string()
}

fn wrap_text_lines(text: &str, max_width_pt: f32, font_size: f32, is_mono: bool) -> Vec<String> {
    let avg_char_w = if is_mono {
        font_size * 0.60
    } else {
        font_size * 0.52
    };
    let max_chars = ((max_width_pt / avg_char_w).floor() as usize).max(12);

    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.chars().count() + 1 + word.chars().count() <= max_chars {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn encode_win_ansi(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| match c {
            '•' => 0x95,
            '–' => 0x96,
            '—' => 0x97,
            '…' => 0x85,
            '\u{0020}'..='\u{00FF}' => c as u8,
            _ => b'?',
        })
        .collect()
}

fn layout_markdown_pages(
    blocks: &[MdBlock],
    page_w: f32,
    page_h: f32,
    margin: f32,
    content_w: f32,
    p: &dyn Progress,
) -> Result<(Vec<Vec<Operation>>, Vec<HeadingEntry>), SheafError> {
    let mut pages: Vec<Vec<Operation>> = vec![Vec::new()];
    let mut headings: Vec<HeadingEntry> = Vec::new();
    let mut cursor_y = page_h - margin;
    let total_blocks = blocks.len().max(1) as u64;

    let ensure_space = |needed: f32, cursor_y: &mut f32, pages: &mut Vec<Vec<Operation>>| {
        if *cursor_y - needed < margin && !pages.last().is_none_or(Vec::is_empty) {
            pages.push(Vec::new());
            *cursor_y = page_h - margin;
        }
    };

    for (idx, block) in blocks.iter().enumerate() {
        if p.is_cancelled() {
            return Err(SheafError::Cancelled);
        }
        p.report((idx as u64 * 90) / total_blocks, 100);

        match block {
            MdBlock::Heading { level, text } => {
                let (font_size, space_before, space_after) = match level {
                    1 => (22.0f32, 14.0f32, 8.0f32),
                    2 => (17.5, 12.0, 6.0),
                    3 => (14.5, 10.0, 5.0),
                    4 => (12.5, 8.0, 4.0),
                    _ => (11.5, 6.0, 4.0),
                };
                let line_h = font_size * 1.3;
                let wrapped = wrap_text_lines(text, content_w, font_size, false);
                let total_h = space_before + (wrapped.len() as f32) * line_h + space_after;
                ensure_space(total_h, &mut cursor_y, &mut pages);

                if cursor_y < page_h - margin - 1.0 {
                    cursor_y -= space_before;
                }
                let page_index = pages.len() - 1;
                headings.push(HeadingEntry {
                    level: *level,
                    title: text.clone(),
                    page_index,
                });

                for line in wrapped {
                    cursor_y -= font_size;
                    if let Some(page_ops) = pages.last_mut() {
                        emit_text_line(page_ops, "F2", font_size, margin, cursor_y, &line);
                    }
                    cursor_y -= line_h - font_size;
                }
                cursor_y -= space_after;
            }
            MdBlock::Paragraph { text } => {
                let font_size = 11.0f32;
                let line_h = 15.5f32;
                let wrapped = wrap_text_lines(text, content_w, font_size, false);
                for line in wrapped {
                    ensure_space(line_h, &mut cursor_y, &mut pages);
                    cursor_y -= font_size;
                    if let Some(page_ops) = pages.last_mut() {
                        emit_text_line(page_ops, "F1", font_size, margin, cursor_y, &line);
                    }
                    cursor_y -= line_h - font_size;
                }
                cursor_y -= 6.0;
            }
            MdBlock::ListItem { marker, text } => {
                let font_size = 11.0f32;
                let line_h = 15.0f32;
                let indent = 18.0f32;
                let wrapped = wrap_text_lines(text, content_w - indent, font_size, false);
                for (line_idx, line) in wrapped.into_iter().enumerate() {
                    ensure_space(line_h, &mut cursor_y, &mut pages);
                    cursor_y -= font_size;
                    if let Some(page_ops) = pages.last_mut() {
                        if line_idx == 0 {
                            emit_text_line(page_ops, "F2", font_size, margin, cursor_y, marker);
                        }
                        emit_text_line(page_ops, "F1", font_size, margin + indent, cursor_y, &line);
                    }
                    cursor_y -= line_h - font_size;
                }
                cursor_y -= 3.0;
            }
            MdBlock::Blockquote { text } => {
                let font_size = 10.5f32;
                let line_h = 14.5f32;
                let indent = 14.0f32;
                let wrapped = wrap_text_lines(text, content_w - indent, font_size, false);
                let block_h = (wrapped.len() as f32) * line_h + 4.0;
                ensure_space(block_h, &mut cursor_y, &mut pages);
                let top_y = cursor_y;
                for line in wrapped {
                    cursor_y -= font_size;
                    if let Some(page_ops) = pages.last_mut() {
                        emit_text_line(page_ops, "F3", font_size, margin + indent, cursor_y, &line);
                    }
                    cursor_y -= line_h - font_size;
                }
                let bottom_y = cursor_y;
                if let Some(page_ops) = pages.last_mut() {
                    page_ops.push(Operation::new("q", vec![]));
                    page_ops.push(Operation::new(
                        "RG",
                        vec![Object::Real(0.70), Object::Real(0.70), Object::Real(0.68)],
                    ));
                    page_ops.push(Operation::new("w", vec![Object::Real(2.0)]));
                    page_ops.push(Operation::new(
                        "m",
                        vec![Object::Real(margin + 4.0), Object::Real(top_y)],
                    ));
                    page_ops.push(Operation::new(
                        "l",
                        vec![Object::Real(margin + 4.0), Object::Real(bottom_y)],
                    ));
                    page_ops.push(Operation::new("S", vec![]));
                    page_ops.push(Operation::new("Q", vec![]));
                }
                cursor_y -= 6.0;
            }
            MdBlock::CodeBlock { lines } => {
                let font_size = 9.5f32;
                let line_h = 13.0f32;
                let pad = 8.0f32;
                let mut wrapped_code = Vec::new();
                for l in lines {
                    if l.trim().is_empty() {
                        wrapped_code.push(String::new());
                    } else {
                        wrapped_code.extend(wrap_text_lines(
                            l,
                            content_w - 2.0 * pad,
                            font_size,
                            true,
                        ));
                    }
                }
                let box_h = (wrapped_code.len() as f32) * line_h + 2.0 * pad;
                ensure_space(box_h.min(page_h - 2.0 * margin), &mut cursor_y, &mut pages);

                let box_top = cursor_y;
                let box_bottom = (box_top - box_h).max(margin);
                if let Some(page_ops) = pages.last_mut() {
                    page_ops.push(Operation::new("q", vec![]));
                    page_ops.push(Operation::new(
                        "rg",
                        vec![Object::Real(0.95), Object::Real(0.95), Object::Real(0.94)],
                    ));
                    page_ops.push(Operation::new(
                        "re",
                        vec![
                            Object::Real(margin),
                            Object::Real(box_bottom),
                            Object::Real(content_w),
                            Object::Real(box_top - box_bottom),
                        ],
                    ));
                    page_ops.push(Operation::new("f", vec![]));
                    page_ops.push(Operation::new("Q", vec![]));
                }

                cursor_y -= pad;
                for line in wrapped_code {
                    ensure_space(line_h, &mut cursor_y, &mut pages);
                    cursor_y -= font_size;
                    if let Some(page_ops) = pages.last_mut() {
                        emit_text_line(page_ops, "F4", font_size, margin + pad, cursor_y, &line);
                    }
                    cursor_y -= line_h - font_size;
                }
                cursor_y -= pad + 6.0;
            }
            MdBlock::HorizontalRule => {
                ensure_space(16.0, &mut cursor_y, &mut pages);
                cursor_y -= 6.0;
                if let Some(page_ops) = pages.last_mut() {
                    page_ops.push(Operation::new("q", vec![]));
                    page_ops.push(Operation::new(
                        "RG",
                        vec![Object::Real(0.82), Object::Real(0.82), Object::Real(0.80)],
                    ));
                    page_ops.push(Operation::new("w", vec![Object::Real(0.75)]));
                    page_ops.push(Operation::new(
                        "m",
                        vec![Object::Real(margin), Object::Real(cursor_y)],
                    ));
                    page_ops.push(Operation::new(
                        "l",
                        vec![Object::Real(page_w - margin), Object::Real(cursor_y)],
                    ));
                    page_ops.push(Operation::new("S", vec![]));
                    page_ops.push(Operation::new("Q", vec![]));
                }
                cursor_y -= 10.0;
            }
        }
    }

    Ok((pages, headings))
}

fn emit_text_line(
    ops: &mut Vec<Operation>,
    font_key: &str,
    font_size: f32,
    x: f32,
    y: f32,
    text: &str,
) {
    ops.push(Operation::new("BT", vec![]));
    ops.push(Operation::new(
        "Tf",
        vec![font_key.into(), Object::Real(font_size)],
    ));
    ops.push(Operation::new("Td", vec![Object::Real(x), Object::Real(y)]));
    ops.push(Operation::new(
        "Tj",
        vec![Object::String(
            encode_win_ansi(text),
            lopdf::StringFormat::Literal,
        )],
    ));
    ops.push(Operation::new("ET", vec![]));
}

fn build_markdown_pdf_document(
    pages_ops: Vec<Vec<Operation>>,
    headings: &[HeadingEntry],
    page_w: f32,
    page_h: f32,
    out_path: &Path,
) -> Result<Document, SheafError> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let f1_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let f2_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica-Bold",
        "Encoding" => "WinAnsiEncoding",
    });
    let f3_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica-Oblique",
        "Encoding" => "WinAnsiEncoding",
    });
    let f4_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier",
        "Encoding" => "WinAnsiEncoding",
    });

    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! {
            "F1" => f1_id,
            "F2" => f2_id,
            "F3" => f3_id,
            "F4" => f4_id,
        },
    });

    let mut page_ids = Vec::with_capacity(pages_ops.len());
    let mut kids = Vec::with_capacity(pages_ops.len());

    for ops in pages_ops {
        let content = Content { operations: ops };
        let stream_bytes = content.encode().map_err(|e| {
            SheafError::InvalidPdf(format!(
                "{}: Failed to encode Markdown PDF content stream: {e}",
                out_path.display()
            ))
        })?;
        let content_id = doc.add_object(Stream::new(dictionary! {}, stream_bytes));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), Object::Real(page_w), Object::Real(page_h)],
            "Resources" => resources_id,
            "Contents" => content_id,
        });
        page_ids.push(page_id);
        kids.push(Object::Reference(page_id));
    }

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => page_ids.len() as i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    // Build PDF Outlines from Markdown headings
    let mut stack: Vec<(u8, u32)> = Vec::new();
    for h in headings {
        if let Some(&target_page_id) = page_ids.get(h.page_index) {
            while let Some(&(top_level, _)) = stack.last() {
                if top_level >= h.level {
                    stack.pop();
                } else {
                    break;
                }
            }
            let parent_bm_id = stack.last().map(|&(_, id)| id);
            let bm = Bookmark::new(h.title.clone(), [0.0, 0.0, 0.0], 0, target_page_id);
            let bm_id = doc.add_bookmark(bm, parent_bm_id);
            stack.push((h.level, bm_id));
        }
    }

    if let Some(outline_id) = doc.build_outline()
        && let Ok(Object::Dictionary(cat)) = doc.get_object_mut(catalog_id)
    {
        cat.set("Outlines", Object::Reference(outline_id));
    }

    Ok(doc)
}
