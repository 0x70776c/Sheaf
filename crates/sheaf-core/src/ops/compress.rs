//! PDF compression operations (`CompressPreset`, `CompressOptions`, `CompressReport`, `compress`, `compress_with_options`).

use std::collections::HashMap;
use std::fs;
use std::io::{BufWriter, Cursor, Write};
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ExtendedColorType, GrayImage, ImageEncoder, ImageReader, RgbImage};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::ensure_not_overwriting_inputs;
use crate::fs::{check_output_path, write_atomic};
use crate::load::{LoadOptions, load_pdf_from_bytes};
use crate::ops::split::sanitize_stem;
use crate::{Progress, SheafError};

/// Compression preset controlling target image DPI, JPEG quality, and deduplication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompressPreset {
    /// Low compression: 220 DPI, JPEG quality 85, structural optimization.
    Low,
    /// Medium compression (default): 150 DPI, JPEG quality 70, object deduplication & image resampling.
    #[default]
    Medium,
    /// High compression: 100 DPI, JPEG quality 55, object deduplication & aggressive resampling.
    High,
}

impl CompressPreset {
    /// Parses a preset name (`"low"`, `"medium"`, `"high"`).
    ///
    /// # Examples
    ///
    /// ```
    /// use sheaf_core::CompressPreset;
    ///
    /// assert_eq!(CompressPreset::parse("medium").unwrap(), CompressPreset::Medium);
    /// assert!(CompressPreset::parse("ultra").is_err());
    /// ```
    pub fn parse(input: &str) -> Result<Self, SheafError> {
        match input.trim().to_ascii_lowercase().as_str() {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            other => Err(SheafError::InvalidSelection(format!(
                "Invalid compression preset '{other}' (expected: low, medium, or high)"
            ))),
        }
    }

    /// Stable lowercase identifier (`"low"`, `"medium"`, `"high"`).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    /// Target image resolution in dots per inch (`220`, `150`, or `100`).
    pub fn target_dpi(&self) -> u32 {
        match self {
            Self::Low => 220,
            Self::Medium => 150,
            Self::High => 100,
        }
    }

    /// Target JPEG quality (`85`, `70`, or `55`).
    pub fn jpeg_quality(&self) -> u8 {
        match self {
            Self::Low => 85,
            Self::Medium => 70,
            Self::High => 55,
        }
    }

    /// Maximum pixel dimension along the longest side for a full-page image at [`Self::target_dpi`].
    fn max_dimension_px(&self) -> u32 {
        // Based on A4/Letter longest edge (~11.69 inches)
        match self {
            Self::Low => 2572,
            Self::Medium => 1754,
            Self::High => 1169,
        }
    }
}

/// Options for [`compress_with_options`].
#[derive(Debug, Clone, Default)]
pub struct CompressOptions {
    /// Loading and decryption options for input PDF.
    pub load: LoadOptions,
    /// Selected compression preset.
    pub preset: CompressPreset,
    /// Convert embedded images to grayscale (`DeviceGray`) during resampling.
    pub grayscale: bool,
    /// Target output PDF path (defaults to `{stem}_compressed.pdf` in the source directory).
    pub out: Option<PathBuf>,
    /// Overwrite existing output file if `true`.
    pub force: bool,
    /// Compute compression result without writing the output file if `true`.
    pub dry_run: bool,
}

/// Report returned by [`compress`] and [`compress_with_options`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompressReport {
    /// Input PDF file path.
    pub input: PathBuf,
    /// Output PDF file path.
    pub output: PathBuf,
    /// Applied preset name (`"low"`, `"medium"`, `"high"`).
    pub preset: &'static str,
    /// Whether grayscale conversion was enabled.
    pub grayscale: bool,
    /// File size of the input PDF in bytes.
    pub size_before_bytes: u64,
    /// File size of the output PDF in bytes.
    pub size_after_bytes: u64,
    /// Bytes saved (`size_before_bytes - size_after_bytes`, or `0` if `already_optimal`).
    pub saved_bytes: u64,
    /// Percentage saved (`0.0` to `100.0`, rounded to 1 decimal place).
    pub saved_percent: f64,
    /// Number of embedded image XObjects resampled/recompressed.
    pub images_resampled: u32,
    /// Number of duplicate stream objects deduplicated.
    pub objects_deduped: u32,
    /// `true` if the PDF could not be reduced further and original bytes were preserved.
    pub already_optimal: bool,
    /// Indicates whether `--dry-run` was active.
    pub dry_run: bool,
    /// Whether the input PDF was originally encrypted.
    #[serde(skip)]
    pub was_encrypted: bool,
}

/// Compresses a PDF file (`src`) using `preset` and writes the result to `out`.
///
/// If the compressed representation is not smaller than `src`, the original bytes are preserved
/// and [`CompressReport::already_optimal`] is set to `true`.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use sheaf_core::{CompressPreset, NoProgress, compress};
///
/// let report = compress(
///     Path::new("photo_doc.pdf"),
///     CompressPreset::Medium,
///     Path::new("photo_doc_compressed.pdf"),
///     &NoProgress,
/// ).unwrap();
/// println!("Saved {:.1}%", report.saved_percent);
/// ```
pub fn compress(
    src: &Path,
    preset: CompressPreset,
    out: &Path,
    p: &dyn Progress,
) -> Result<CompressReport, SheafError> {
    let opts = CompressOptions {
        load: LoadOptions::default(),
        preset,
        grayscale: false,
        out: Some(out.to_path_buf()),
        force: true,
        dry_run: false,
    };
    compress_with_options(src, &opts, p)
}

/// Compresses a PDF file (`src`) according to [`CompressOptions`].
///
/// # Errors
///
/// Returns:
/// - [`SheafError::Io`] if `src` cannot be read or output cannot be written.
/// - [`SheafError::Encrypted`] if `src` is password-protected.
/// - [`SheafError::InvalidPdf`] if `src` is corrupted.
/// - [`SheafError::OutputExists`] if the target output exists and `!opts.force`.
/// - [`SheafError::Cancelled`] if cancelled via `p`.
///
/// # Examples
///
/// ```no_run
/// use std::path::{Path, PathBuf};
/// use sheaf_core::{CompressOptions, CompressPreset, NoProgress, compress_with_options};
///
/// let opts = CompressOptions {
///     preset: CompressPreset::High,
///     grayscale: true,
///     out: Some(PathBuf::from("out.pdf")),
///     force: true,
///     dry_run: false,
///     ..CompressOptions::default()
/// };
/// let report = compress_with_options(Path::new("input.pdf"), &opts, &NoProgress).unwrap();
/// assert!(report.size_after_bytes <= report.size_before_bytes);
/// ```
pub fn compress_with_options(
    src: &Path,
    opts: &CompressOptions,
    p: &dyn Progress,
) -> Result<CompressReport, SheafError> {
    if p.is_cancelled() {
        return Err(SheafError::Cancelled);
    }

    let original_bytes = fs::read(src)?;
    let size_before_bytes = u64::try_from(original_bytes.len()).unwrap_or(0);

    let out_path = resolve_compress_output_path(src, opts.out.as_deref());
    ensure_not_overwriting_inputs(&[src.to_path_buf()], &out_path)?;
    check_output_path(&out_path, opts.force)?;

    p.report(0, 4);

    let loaded = load_pdf_from_bytes(&original_bytes, src, &opts.load)?;
    let was_encrypted = loaded.was_encrypted;
    let mut doc = loaded.doc;

    if doc.get_pages().is_empty() {
        return Err(SheafError::InvalidPdf(format!(
            "{}: document contains 0 pages",
            src.display()
        )));
    }

    if p.is_cancelled() {
        return Err(SheafError::Cancelled);
    }
    p.report(1, 4);

    // Step 1: Deduplicate identical stream objects (Medium & High presets)
    let objects_deduped = if matches!(opts.preset, CompressPreset::Medium | CompressPreset::High) {
        deduplicate_streams(&mut doc)
    } else {
        0
    };

    if p.is_cancelled() {
        return Err(SheafError::Cancelled);
    }
    p.report(2, 4);

    // Step 2: Resample & JPEG-compress embedded Image XObjects
    let images_resampled = resample_document_images(&mut doc, opts.preset, opts.grayscale, p)?;

    if p.is_cancelled() {
        return Err(SheafError::Cancelled);
    }
    p.report(3, 4);

    // Step 3: Prune unused objects, delete empty streams, compress remaining streams, and renumber
    let _ = doc.delete_zero_length_streams();
    let _ = doc.prune_objects();
    doc.compress();
    doc.renumber_objects();

    let mut compressed_bytes = Vec::with_capacity(original_bytes.len());
    doc.save_to(&mut compressed_bytes)
        .map_err(|e| SheafError::Io(std::io::Error::other(e.to_string())))?;

    let candidate_size = u64::try_from(compressed_bytes.len()).unwrap_or(u64::MAX);
    let (final_bytes, size_after_bytes, saved_bytes, saved_percent, already_optimal) =
        if candidate_size < size_before_bytes && size_before_bytes > 0 {
            let saved = size_before_bytes - candidate_size;
            let pct = ((saved as f64 / size_before_bytes as f64) * 1000.0).round() / 10.0;
            (compressed_bytes, candidate_size, saved, pct, false)
        } else {
            (original_bytes, size_before_bytes, 0, 0.0, true)
        };

    if !opts.dry_run {
        write_atomic(&out_path, opts.force, |file| {
            let mut writer = BufWriter::new(file);
            writer.write_all(&final_bytes)?;
            writer.flush()?;
            Ok(())
        })?;
    }

    p.report(4, 4);

    Ok(CompressReport {
        input: src.to_path_buf(),
        output: out_path,
        preset: opts.preset.as_str(),
        grayscale: opts.grayscale,
        size_before_bytes,
        size_after_bytes,
        saved_bytes,
        saved_percent,
        images_resampled,
        objects_deduped,
        already_optimal,
        dry_run: opts.dry_run,
        was_encrypted,
    })
}

fn resolve_compress_output_path(src: &Path, explicit_out: Option<&Path>) -> PathBuf {
    if let Some(out) = explicit_out
        && !out.as_os_str().is_empty()
    {
        return out.to_path_buf();
    }
    let parent = src
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let stem = sanitize_stem(src);
    parent.join(format!("{stem}_compressed.pdf"))
}

fn deduplicate_streams(doc: &mut Document) -> u32 {
    let mut canonical_by_hash: HashMap<[u8; 32], ObjectId> = HashMap::new();
    let mut replacements: HashMap<ObjectId, ObjectId> = HashMap::new();

    let object_ids: Vec<ObjectId> = doc.objects.keys().copied().collect();
    for id in object_ids {
        let Some(Object::Stream(stream)) = doc.objects.get(&id) else {
            continue;
        };
        let mut hasher = Sha256::new();
        // Hash dictionary entries (sorted by key, excluding Length) + content bytes
        let mut keys: Vec<&Vec<u8>> = stream
            .dict
            .iter()
            .map(|(k, _)| k)
            .filter(|k| k.as_slice() != b"Length")
            .collect();
        keys.sort();
        for k in keys {
            hasher.update(k);
            if let Ok(val) = stream.dict.get(k) {
                hasher.update(format!("{val:?}").as_bytes());
            }
        }
        hasher.update(b"|");
        hasher.update(&stream.content);
        let digest: [u8; 32] = hasher.finalize().into();

        if let Some(&existing_id) = canonical_by_hash.get(&digest) {
            if existing_id != id {
                replacements.insert(id, existing_id);
            }
        } else {
            canonical_by_hash.insert(digest, id);
        }
    }

    if replacements.is_empty() {
        return 0;
    }

    for object in doc.objects.values_mut() {
        rewrite_object_references(object, &replacements);
    }
    for (_, val) in doc.trailer.iter_mut() {
        rewrite_object_references(val, &replacements);
    }

    let mut removed = 0u32;
    for dup_id in replacements.keys() {
        if doc.objects.remove(dup_id).is_some() {
            removed = removed.saturating_add(1);
        }
    }
    removed
}

fn rewrite_object_references(obj: &mut Object, map: &HashMap<ObjectId, ObjectId>) {
    match obj {
        Object::Reference(id) => {
            if let Some(&new_id) = map.get(id) {
                *id = new_id;
            }
        }
        Object::Array(arr) => {
            for item in arr {
                rewrite_object_references(item, map);
            }
        }
        Object::Dictionary(dict) => {
            for (_, val) in dict.iter_mut() {
                rewrite_object_references(val, map);
            }
        }
        Object::Stream(stream) => {
            for (_, val) in stream.dict.iter_mut() {
                rewrite_object_references(val, map);
            }
        }
        _ => {}
    }
}

fn resample_document_images(
    doc: &mut Document,
    preset: CompressPreset,
    grayscale: bool,
    p: &dyn Progress,
) -> Result<u32, SheafError> {
    let max_dim = preset.max_dimension_px();
    let quality = preset.jpeg_quality();
    let object_ids: Vec<ObjectId> = doc.objects.keys().copied().collect();
    let mut resampled_count = 0u32;

    for id in object_ids {
        if p.is_cancelled() {
            return Err(SheafError::Cancelled);
        }

        let Some(Object::Stream(stream)) = doc.objects.get(&id) else {
            continue;
        };

        if !is_resampleable_image_xobject(&stream.dict) {
            continue;
        }

        let Some(decoded_img) = decode_pdf_image_stream(stream) else {
            continue;
        };

        let orig_w = decoded_img.width();
        let orig_h = decoded_img.height();
        if orig_w == 0 || orig_h == 0 {
            continue;
        }

        let longest = orig_w.max(orig_h);
        let resized = if longest > max_dim {
            let scale = (max_dim as f64) / (longest as f64);
            let new_w = ((orig_w as f64) * scale).round().max(1.0) as u32;
            let new_h = ((orig_h as f64) * scale).round().max(1.0) as u32;
            decoded_img.resize_exact(new_w, new_h, FilterType::Lanczos3)
        } else {
            decoded_img
        };

        let out_w = resized.width();
        let out_h = resized.height();

        let is_gray = grayscale || matches!(resized, DynamicImage::ImageLuma8(_));
        let mut jpeg_bytes = Vec::new();
        let encoder = JpegEncoder::new_with_quality(Cursor::new(&mut jpeg_bytes), quality);

        let encode_res = if is_gray {
            let luma = resized.to_luma8();
            encoder.write_image(luma.as_raw(), out_w, out_h, ExtendedColorType::L8)
        } else {
            let rgb = resized.to_rgb8();
            encoder.write_image(rgb.as_raw(), out_w, out_h, ExtendedColorType::Rgb8)
        };

        if encode_res.is_err() {
            continue;
        }

        // Only replace the stream if the new JPEG stream is smaller than the existing stream content
        if jpeg_bytes.len() < stream.content.len()
            && let Some(Object::Stream(mut_stream)) = doc.objects.get_mut(&id)
        {
            mut_stream.dict.set("Width", i64::from(out_w));
            mut_stream.dict.set("Height", i64::from(out_h));
            mut_stream.dict.set(
                "ColorSpace",
                Object::Name(if is_gray {
                    b"DeviceGray".to_vec()
                } else {
                    b"DeviceRGB".to_vec()
                }),
            );
            mut_stream.dict.set("BitsPerComponent", 8i64);
            mut_stream
                .dict
                .set("Filter", Object::Name(b"DCTDecode".to_vec()));
            mut_stream.dict.remove(b"DecodeParms");
            mut_stream.allows_compression = false;
            mut_stream.set_content(jpeg_bytes);
            resampled_count = resampled_count.saturating_add(1);
        }
    }

    Ok(resampled_count)
}

fn is_resampleable_image_xobject(dict: &Dictionary) -> bool {
    let Ok(subtype) = dict.get(b"Subtype").and_then(Object::as_name) else {
        return false;
    };
    if subtype != b"Image" {
        return false;
    }
    // Skip images with alpha/soft masks or 1-bit masks to preserve transparency and stencils
    if dict.has(b"SMask") || dict.has(b"Mask") || dict.has(b"ImageMask") {
        return false;
    }
    let bpc = dict
        .get(b"BitsPerComponent")
        .and_then(Object::as_i64)
        .unwrap_or(8);
    bpc == 8
}

fn decode_pdf_image_stream(stream: &Stream) -> Option<DynamicImage> {
    let width = u32::try_from(stream.dict.get(b"Width").and_then(Object::as_i64).ok()?).ok()?;
    let height = u32::try_from(stream.dict.get(b"Height").and_then(Object::as_i64).ok()?).ok()?;
    let color_space = stream
        .dict
        .get(b"ColorSpace")
        .and_then(Object::as_name)
        .ok()?;

    let filter = stream.dict.get(b"Filter").and_then(Object::as_name).ok();

    if filter == Some(b"DCTDecode".as_slice()) {
        let reader = ImageReader::new(Cursor::new(&stream.content))
            .with_guessed_format()
            .ok()?;
        return reader.decode().ok();
    }

    let raw_pixels = if filter.is_none() {
        stream.content.clone()
    } else {
        stream.decompressed_content().ok()?
    };

    if color_space == b"DeviceRGB" {
        let expected_len = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(3)?;
        if raw_pixels.len() != expected_len {
            return None;
        }
        RgbImage::from_raw(width, height, raw_pixels).map(DynamicImage::ImageRgb8)
    } else if color_space == b"DeviceGray" {
        let expected_len = (width as usize).checked_mul(height as usize)?;
        if raw_pixels.len() != expected_len {
            return None;
        }
        GrayImage::from_raw(width, height, raw_pixels).map(DynamicImage::ImageLuma8)
    } else {
        None
    }
}
