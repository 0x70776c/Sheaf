//! PDF verification (`verify`).

use std::path::{Path, PathBuf};
use std::process::Command;

use lopdf::Document;
use serde::Serialize;

use crate::{Progress, SheafError};

/// Options for PDF verification.
#[derive(Debug, Clone, Default)]
pub struct VerifyOptions {
    /// Run `qpdf --check` in addition to structural `lopdf` checks when available.
    pub deep: bool,
}

/// Verification result for a single PDF file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifyFileResult {
    /// Path to the inspected PDF file.
    pub path: PathBuf,
    /// Whether the file passed all verification checks.
    pub valid: bool,
    /// Number of pages if readable (`None` if unreadable).
    pub pages: Option<u32>,
    /// Structural or `qpdf` problems found in the file.
    pub problems: Vec<String>,
}

/// Report returned by [`verify`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifyReport {
    /// Verification results per input file.
    pub files: Vec<VerifyFileResult>,
    /// Non-fatal warnings (e.g. if `--deep` was requested but `qpdf` is not installed).
    #[serde(skip)]
    pub warnings: Vec<String>,
}

impl VerifyReport {
    /// Returns `true` if every inspected file is valid.
    pub fn all_valid(&self) -> bool {
        self.files.iter().all(|f| f.valid)
    }
}

/// Returns the path/name of the `qpdf` executable if available on the system.
///
/// # Examples
///
/// ```
/// use sheaf_core::ops::verify::find_qpdf;
///
/// let _ = find_qpdf();
/// ```
pub fn find_qpdf() -> Option<&'static str> {
    ["qpdf", "/opt/homebrew/bin/qpdf", "/usr/local/bin/qpdf"]
        .into_iter()
        .find(|bin| {
            Command::new(bin)
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        })
}

/// Verifies one or more PDF files for structural integrity.
///
/// Checks that every file loads cleanly, has a traversable `/Pages` tree with `pages > 0`,
/// and that every page object and its `/Contents` references resolve inside the document.
/// When `opts.deep` is `true`, also invokes `qpdf --check` if available.
///
/// All files are checked even if an earlier file is invalid.
///
/// # Errors
///
/// Returns:
/// - [`SheafError::EmptySelection`] if `files` is empty.
/// - [`SheafError::Io`] if a file does not exist on disk.
/// - [`SheafError::Cancelled`] if cancelled via `p`.
///
/// # Examples
///
/// ```no_run
/// use std::path::PathBuf;
/// use sheaf_core::ops::verify::{verify, VerifyOptions};
/// use sheaf_core::NoProgress;
///
/// let report = verify(&[PathBuf::from("sample.pdf")], &VerifyOptions::default(), &NoProgress).unwrap();
/// assert!(report.all_valid());
/// ```
pub fn verify(
    files: &[PathBuf],
    opts: &VerifyOptions,
    p: &dyn Progress,
) -> Result<VerifyReport, SheafError> {
    if files.is_empty() {
        return Err(SheafError::EmptySelection);
    }

    for path in files {
        if !path.exists() {
            return Err(SheafError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("File not found: {}", path.display()),
            )));
        }
    }

    let mut warnings = Vec::new();
    let qpdf_bin = if opts.deep {
        let found = find_qpdf();
        if found.is_none() {
            warnings.push(
                "qpdf not found in PATH – running internal structural check only.".to_string(),
            );
        }
        found
    } else {
        None
    };

    let total = files.len() as u64;
    p.report(0, total);

    let mut results = Vec::with_capacity(files.len());
    for (idx, path) in files.iter().enumerate() {
        if p.is_cancelled() {
            return Err(SheafError::Cancelled);
        }
        results.push(verify_single_file(path, qpdf_bin));
        p.report((idx as u64) + 1, total);
    }

    Ok(VerifyReport {
        files: results,
        warnings,
    })
}

fn verify_single_file(path: &Path, qpdf_bin: Option<&str>) -> VerifyFileResult {
    let mut problems = Vec::new();

    let doc = match Document::load(path) {
        Ok(d) => d,
        Err(e) => {
            problems.push(format!("Failed to load PDF: {e}"));
            return VerifyFileResult {
                path: path.to_path_buf(),
                valid: false,
                pages: None,
                problems,
            };
        }
    };

    if doc.is_encrypted() {
        problems.push("Document is encrypted.".to_string());
        return VerifyFileResult {
            path: path.to_path_buf(),
            valid: false,
            pages: None,
            problems,
        };
    }

    if let Err(e) = doc.catalog() {
        problems.push(format!("Catalog missing or invalid: {e}"));
    }

    let pages = doc.get_pages();
    if pages.is_empty() {
        problems.push("Document contains no pages (page count = 0).".to_string());
    }

    for (page_num, page_id) in &pages {
        match doc.get_dictionary(*page_id) {
            Ok(page_dict) => {
                for content_id in doc.get_page_contents(*page_id) {
                    if doc.get_object(content_id).is_err() {
                        problems.push(format!(
                            "Page {page_num}: content stream {content_id:?} could not be resolved"
                        ));
                    }
                }
                if let Ok(res_ref) = page_dict.get(b"Resources").and_then(|o| o.as_reference())
                    && doc.get_object(res_ref).is_err()
                {
                    problems.push(format!(
                        "Page {page_num}: Resources object {res_ref:?} could not be resolved"
                    ));
                }
            }
            Err(e) => {
                problems.push(format!(
                    "Page {page_num} ({page_id:?}) is not a valid dictionary: {e}"
                ));
            }
        }
    }

    if let Some(qpdf) = qpdf_bin {
        match Command::new(qpdf).arg("--check").arg(path).output() {
            Ok(out) => {
                let code = out.status.code().unwrap_or(-1);
                if code != 0 && code != 3 {
                    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    let detail = if !stderr.is_empty() { stderr } else { stdout };
                    problems.push(format!("qpdf --check failed (exit {code}): {detail}"));
                }
            }
            Err(e) => {
                problems.push(format!("qpdf invocation failed: {e}"));
            }
        }
    }

    let page_count = u32::try_from(pages.len()).ok().filter(|&n| n > 0);
    VerifyFileResult {
        path: path.to_path_buf(),
        valid: problems.is_empty(),
        pages: page_count,
        problems,
    }
}
