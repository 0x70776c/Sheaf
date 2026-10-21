//! PDF password protection and unlocking operations (`ProtectOptions`, `UnlockOptions`, `ProtectReport`, `protect`, `unlock`).

use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use lopdf::{Document, EncryptionState, EncryptionVersion, LoadOptions, Object, Permissions};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::ensure_not_overwriting_inputs;
use crate::fs::{check_output_path, write_atomic};
use crate::ops::split::sanitize_stem;
use crate::{Progress, SheafError};

/// Options for [`protect`].
#[derive(Debug, Clone)]
pub struct ProtectOptions {
    /// User password required to open the encrypted PDF.
    pub user_password: String,
    /// Optional owner password (defaults to `user_password` when `None`).
    pub owner_password: Option<String>,
    /// Allow printing the document (`true` by default).
    pub allow_print: bool,
    /// Allow copying text and graphics (`true` by default).
    pub allow_copy: bool,
    /// Target output PDF path (defaults to `{stem}_protected.pdf`).
    pub out: Option<PathBuf>,
    /// Overwrite existing output file if `true`.
    pub force: bool,
    /// Compute protection plan without writing the output file if `true`.
    pub dry_run: bool,
}

impl Default for ProtectOptions {
    fn default() -> Self {
        Self {
            user_password: String::new(),
            owner_password: None,
            allow_print: true,
            allow_copy: true,
            out: None,
            force: false,
            dry_run: false,
        }
    }
}

/// Options for [`unlock`].
#[derive(Debug, Clone, Default)]
pub struct UnlockOptions {
    /// User or owner password to unlock the encrypted PDF.
    pub password: String,
    /// Target output PDF path (defaults to `{stem}_unlocked.pdf`).
    pub out: Option<PathBuf>,
    /// Overwrite existing output file if `true`.
    pub force: bool,
    /// Validate password without writing the output file if `true`.
    pub dry_run: bool,
}

/// Report returned by [`protect`] and [`unlock`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProtectReport {
    /// Operation performed (`"protect"` or `"unlock"`).
    pub action: &'static str,
    /// Input PDF file path.
    pub input: PathBuf,
    /// Output PDF file path.
    pub output: PathBuf,
    /// Total pages in the document.
    pub pages: u32,
    /// Whether the output document is encrypted (`true` for `protect`, `false` for `unlock`).
    pub encrypted: bool,
    /// Indicates whether `--dry-run` was active.
    pub dry_run: bool,
}

/// Encrypts a PDF file (`src`) with a password and writes the protected PDF to `opts.out`.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use sheaf_core::{NoProgress, ProtectOptions, protect};
///
/// let opts = ProtectOptions {
///     user_password: "secret-password".to_string(),
///     ..ProtectOptions::default()
/// };
/// let report = protect(Path::new("report.pdf"), &opts, &NoProgress).unwrap();
/// assert!(report.encrypted);
/// ```
pub fn protect(
    src: &Path,
    opts: &ProtectOptions,
    p: &dyn Progress,
) -> Result<ProtectReport, SheafError> {
    if opts.user_password.is_empty() {
        return Err(SheafError::InvalidSelection(
            "Password cannot be empty".to_string(),
        ));
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
            parent.join(format!("{stem}_protected.pdf"))
        }
    };
    ensure_not_overwriting_inputs(&[src.to_path_buf()], &out_path)?;
    if !opts.dry_run {
        check_output_path(&out_path, opts.force)?;
    }

    if p.is_cancelled() {
        return Err(SheafError::Cancelled);
    }
    p.report(10, 100);

    let raw_bytes = fs::read(src)?;
    let mut doc =
        Document::load_mem(&raw_bytes).map_err(|e| SheafError::from_lopdf_with_path(e, src))?;

    if doc.is_encrypted() || doc.trailer.get(b"Encrypt").is_ok() {
        return Err(SheafError::Encrypted);
    }

    let pages_count = doc.get_pages().len() as u32;
    if pages_count == 0 {
        return Err(SheafError::InvalidPdf(format!(
            "{}: Document contains 0 pages",
            src.display()
        )));
    }

    if p.is_cancelled() {
        return Err(SheafError::Cancelled);
    }
    p.report(50, 100);

    if !opts.dry_run {
        ensure_document_id(&mut doc, src, raw_bytes.len());

        let mut perms = Permissions::empty();
        if opts.allow_print {
            perms |= Permissions::PRINTABLE | Permissions::PRINTABLE_IN_HIGH_QUALITY;
        }
        if opts.allow_copy {
            perms |= Permissions::COPYABLE | Permissions::COPYABLE_FOR_ACCESSIBILITY;
        }

        let owner_pw = opts
            .owner_password
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(&opts.user_password);

        let state = EncryptionState::try_from(EncryptionVersion::V2 {
            document: &doc,
            owner_password: owner_pw,
            user_password: &opts.user_password,
            key_length: 128,
            permissions: perms,
        })
        .map_err(|e| SheafError::InvalidPdf(format!("Failed to initialize encryption: {e}")))?;

        doc.encrypt(&state)
            .map_err(|e| SheafError::InvalidPdf(format!("Failed to encrypt document: {e}")))?;

        write_atomic(&out_path, opts.force, |file| {
            let mut writer = BufWriter::new(file);
            doc.save_to(&mut writer)
                .map_err(|e| std::io::Error::other(format!("Failed to write PDF: {e}")))?;
            writer.flush()?;
            Ok(())
        })?;
    }

    p.report(100, 100);

    Ok(ProtectReport {
        action: "protect",
        input: src.to_path_buf(),
        output: out_path,
        pages: pages_count,
        encrypted: true,
        dry_run: opts.dry_run,
    })
}

/// Unlocks (decrypts) a password-protected PDF file (`src`) using `opts.password`
/// and writes the decrypted PDF to `opts.out`.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use sheaf_core::{NoProgress, UnlockOptions, unlock};
///
/// let opts = UnlockOptions {
///     password: "secret-password".to_string(),
///     ..UnlockOptions::default()
/// };
/// let report = unlock(Path::new("report_protected.pdf"), &opts, &NoProgress).unwrap();
/// assert!(!report.encrypted);
/// ```
pub fn unlock(
    src: &Path,
    opts: &UnlockOptions,
    p: &dyn Progress,
) -> Result<ProtectReport, SheafError> {
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
            parent.join(format!("{stem}_unlocked.pdf"))
        }
    };
    ensure_not_overwriting_inputs(&[src.to_path_buf()], &out_path)?;
    if !opts.dry_run {
        check_output_path(&out_path, opts.force)?;
    }

    if p.is_cancelled() {
        return Err(SheafError::Cancelled);
    }
    p.report(10, 100);

    let raw_bytes = fs::read(src)?;

    // Check if the document is actually encrypted before attempting password unlock
    if let Ok(probe_doc) = Document::load_mem(&raw_bytes)
        && !probe_doc.is_encrypted()
        && probe_doc.trailer.get(b"Encrypt").is_err()
        && !raw_bytes.windows(8).any(|w| w == b"/Encrypt")
    {
        return Err(SheafError::InvalidSelection(
            "Document is not password-protected".to_string(),
        ));
    }

    let mut doc =
        Document::load_mem_with_options(&raw_bytes, LoadOptions::with_password(&opts.password))
            .map_err(|e| match e {
                lopdf::Error::Decryption(_) | lopdf::Error::InvalidPassword => {
                    SheafError::WrongPassword
                }
                other => SheafError::from_lopdf_with_path(other, src),
            })?;

    if doc.is_encrypted() {
        doc.decrypt(&opts.password)
            .map_err(|_| SheafError::WrongPassword)?;
    }

    doc.trailer.remove(b"Encrypt");
    doc.encryption_state = None;

    let pages_count = doc.get_pages().len() as u32;
    if pages_count == 0 {
        return Err(SheafError::WrongPassword);
    }

    if p.is_cancelled() {
        return Err(SheafError::Cancelled);
    }
    p.report(60, 100);

    if !opts.dry_run {
        doc.compress();
        write_atomic(&out_path, opts.force, |file| {
            let mut writer = BufWriter::new(file);
            doc.save_to(&mut writer)
                .map_err(|e| std::io::Error::other(format!("Failed to write PDF: {e}")))?;
            writer.flush()?;
            Ok(())
        })?;
    }

    p.report(100, 100);

    Ok(ProtectReport {
        action: "unlock",
        input: src.to_path_buf(),
        output: out_path,
        pages: pages_count,
        encrypted: false,
        dry_run: opts.dry_run,
    })
}

fn ensure_document_id(doc: &mut Document, src: &Path, byte_len: usize) {
    let needs_id = match doc.trailer.get(b"ID") {
        Ok(Object::Array(arr)) => arr.len() < 2,
        _ => true,
    };
    if needs_id {
        let mut hasher = Sha256::new();
        hasher.update(src.to_string_lossy().as_bytes());
        hasher.update(byte_len.to_le_bytes());
        let digest = hasher.finalize();
        let id16 = digest[..16].to_vec();
        doc.trailer.set(
            "ID",
            Object::Array(vec![
                Object::string_literal(id16.clone()),
                Object::string_literal(id16),
            ]),
        );
    }
}
