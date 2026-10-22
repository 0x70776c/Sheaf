//! PDF document inspection (`info`).

use std::path::{Path, PathBuf};

use lopdf::{Dictionary, Document, Object};
use serde::Serialize;

use crate::load::{LoadOptions, load_pdf};
use crate::{Progress, SheafError};

/// Metadata dictionary fields of a PDF document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PdfMetadataInfo {
    /// Document title (`/Title`).
    pub title: Option<String>,
    /// Document author (`/Author`).
    pub author: Option<String>,
    /// Document subject (`/Subject`).
    pub subject: Option<String>,
    /// Document keywords (`/Keywords`).
    pub keywords: Option<String>,
    /// Creator application (`/Creator`).
    pub creator: Option<String>,
    /// PDF producer (`/Producer`).
    pub producer: Option<String>,
}

/// Permissions decoded from the PDF encryption dictionary (`/P`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EncryptionPermissionsInfo {
    /// Whether printing is permitted.
    pub print: bool,
    /// Whether text and graphics copying/extraction is permitted.
    pub copy: bool,
    /// Whether modifying contents, annotations, and forms is permitted.
    pub modify: bool,
}

/// Details about the PDF document encryption.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EncryptionInfo {
    /// Encryption algorithm (`"aes-128"`, `"aes-256"`, or `"rc4"`).
    pub algorithm: String,
    /// Encryption key length in bits (e.g. 128 or 256).
    pub key_bits: u32,
    /// Encryption revision number (e.g. 4, 5, or 6).
    pub revision: i64,
    /// Decoded permission flags.
    pub permissions: EncryptionPermissionsInfo,
}

/// Information about a single PDF file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FileInfo {
    /// Path to the PDF file.
    pub path: PathBuf,
    /// File size in bytes.
    pub size_bytes: u64,
    /// PDF specification version (e.g. `"1.5"`).
    pub pdf_version: String,
    /// Number of pages (`None` when the file is encrypted and unreadable).
    pub pages: Option<u32>,
    /// Whether the file is encrypted / password-protected.
    pub encrypted: bool,
    /// Encryption details (`None` when unencrypted).
    pub encryption: Option<EncryptionInfo>,
    /// Document metadata (`None` when encrypted or when no Info dictionary is present).
    pub metadata: Option<PdfMetadataInfo>,
    /// Whether the document has bookmarks / outlines.
    pub has_outlines: bool,
    /// Number of bookmark / outline entries.
    pub outline_count: usize,
    /// Dimensions `[width_pt, height_pt]` of the first page in points (`None` when encrypted or missing).
    pub first_page_size_pt: Option<[f64; 2]>,
}

/// Report returned by [`info`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InfoReport {
    /// Inspected PDF files.
    pub files: Vec<FileInfo>,
}

/// Inspects one or more PDF files and returns their structural and metadata properties.
///
/// Encrypted PDF files are reported with `encrypted: true` and `pages: None` (without returning
/// [`SheafError::Encrypted`]). Corrupted files return [`SheafError::InvalidPdf`].
///
/// # Errors
///
/// Returns:
/// - [`SheafError::EmptySelection`] if `files` is empty.
/// - [`SheafError::Io`] if a file does not exist or cannot be read.
/// - [`SheafError::InvalidPdf`] if a file is not a valid PDF.
/// - [`SheafError::Cancelled`] if cancelled via `p`.
///
/// # Examples
///
/// ```no_run
/// use std::path::PathBuf;
/// use sheaf_core::{info, NoProgress};
///
/// let report = info(&[PathBuf::from("sample.pdf")], &NoProgress).unwrap();
/// assert_eq!(report.files.len(), 1);
/// ```
pub fn info(files: &[PathBuf], p: &dyn Progress) -> Result<InfoReport, SheafError> {
    info_with_options(files, &LoadOptions::none(), p)
}

/// Inspects one or more local PDF files with specified loading/decryption options.
pub fn info_with_options(
    files: &[PathBuf],
    load_opts: &LoadOptions,
    p: &dyn Progress,
) -> Result<InfoReport, SheafError> {
    if files.is_empty() {
        return Err(SheafError::EmptySelection);
    }

    let total = files.len() as u64;
    p.report(0, total);

    let mut infos = Vec::with_capacity(files.len());
    for (idx, path) in files.iter().enumerate() {
        if p.is_cancelled() {
            return Err(SheafError::Cancelled);
        }
        infos.push(inspect_single_file(path, load_opts)?);
        p.report((idx as u64) + 1, total);
    }

    Ok(InfoReport { files: infos })
}

fn inspect_single_file(path: &Path, load_opts: &LoadOptions) -> Result<FileInfo, SheafError> {
    let fs_meta = std::fs::metadata(path)?;
    let size_bytes = fs_meta.len();
    let raw_bytes = std::fs::read(path)?;

    let loaded = load_pdf(path, load_opts);

    let (doc, was_encrypted) = match loaded {
        Ok(l) => (l.doc, l.was_encrypted),
        Err(SheafError::Encrypted | SheafError::WrongPassword) => {
            let version = read_header_version(path).unwrap_or_else(|| "1.4".to_string());
            let encryption = crate::load::extract_encryption_info(&raw_bytes, None);
            return Ok(FileInfo {
                path: path.to_path_buf(),
                size_bytes,
                pdf_version: version,
                pages: None,
                encrypted: true,
                encryption,
                metadata: None,
                has_outlines: false,
                outline_count: 0,
                first_page_size_pt: None,
            });
        }
        Err(e) => return Err(e),
    };

    let encryption = if was_encrypted {
        crate::load::extract_encryption_info(&raw_bytes, Some(&doc))
    } else {
        None
    };

    if was_encrypted && doc.is_encrypted() {
        return Ok(FileInfo {
            path: path.to_path_buf(),
            size_bytes,
            pdf_version: doc.version.clone(),
            pages: None,
            encrypted: true,
            encryption,
            metadata: None,
            has_outlines: false,
            outline_count: 0,
            first_page_size_pt: None,
        });
    }

    let pages_map = doc.get_pages();
    if pages_map.is_empty() {
        return Err(SheafError::InvalidPdf(format!(
            "{}: document contains no pages",
            path.display()
        )));
    }

    let page_count = u32::try_from(pages_map.len()).unwrap_or(u32::MAX);
    let metadata = extract_metadata(&doc);

    let (has_outlines, outline_count) = match doc.get_toc() {
        Ok(toc) if !toc.toc.is_empty() => (true, toc.toc.len()),
        _ => (false, 0),
    };

    let first_page_size_pt = pages_map
        .values()
        .next()
        .and_then(|&first_id| extract_page_size_pt(&doc, first_id));

    Ok(FileInfo {
        path: path.to_path_buf(),
        size_bytes,
        pdf_version: doc.version.clone(),
        pages: Some(page_count),
        encrypted: was_encrypted,
        encryption,
        metadata,
        has_outlines,
        outline_count,
        first_page_size_pt,
    })
}

fn read_header_version(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let prefix = bytes.get(..16).unwrap_or(&bytes);
    let header = std::str::from_utf8(prefix).ok()?;
    let rest = header.strip_prefix("%PDF-")?;
    let ver: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if ver.is_empty() { None } else { Some(ver) }
}

fn extract_metadata(doc: &Document) -> Option<PdfMetadataInfo> {
    let info_obj = doc.trailer.get(b"Info").ok()?;
    let dict = match info_obj {
        Object::Dictionary(d) => d,
        Object::Reference(id) => doc.get_dictionary(*id).ok()?,
        _ => return None,
    };

    Some(PdfMetadataInfo {
        title: dict_string_field(doc, dict, b"Title"),
        author: dict_string_field(doc, dict, b"Author"),
        subject: dict_string_field(doc, dict, b"Subject"),
        keywords: dict_string_field(doc, dict, b"Keywords"),
        creator: dict_string_field(doc, dict, b"Creator"),
        producer: dict_string_field(doc, dict, b"Producer"),
    })
}

fn dict_string_field(doc: &Document, dict: &Dictionary, key: &[u8]) -> Option<String> {
    let obj = dict.get(key).ok()?;
    let resolved = match obj {
        Object::Reference(id) => doc.get_object(*id).ok()?,
        other => other,
    };
    lopdf::decode_text_string(resolved).ok()
}

fn extract_page_size_pt(doc: &Document, page_id: lopdf::ObjectId) -> Option<[f64; 2]> {
    let mut current_id = Some(page_id);
    let mut depth = 0usize;
    while let Some(id) = current_id {
        if depth > 32 {
            break;
        }
        depth += 1;
        let dict = doc.get_dictionary(id).ok()?;
        if let Ok(mb) = dict.get_deref(b"MediaBox", doc)
            && let Ok(arr) = mb.as_array()
            && arr.len() == 4
        {
            let x0 = object_to_f64(&arr[0])?;
            let y0 = object_to_f64(&arr[1])?;
            let x1 = object_to_f64(&arr[2])?;
            let y1 = object_to_f64(&arr[3])?;
            return Some([(x1 - x0).abs(), (y1 - y0).abs()]);
        }
        current_id = dict.get(b"Parent").ok().and_then(|p| p.as_reference().ok());
    }
    None
}

fn object_to_f64(obj: &Object) -> Option<f64> {
    match obj {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(f64::from(*r)),
        _ => None,
    }
}
