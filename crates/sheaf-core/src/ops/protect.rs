//! PDF password protection and unlocking operations (`ProtectOptions`, `UnlockOptions`, `ProtectReport`, `protect`, `unlock`).

use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use lopdf::{Document, EncryptionState, EncryptionVersion, Object, Permissions};
use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::ensure_not_overwriting_inputs;
use crate::fs::{check_output_path, write_atomic};
use crate::load::{LoadOptions, load_pdf, load_pdf_from_bytes};
use crate::ops::info::EncryptionPermissionsInfo;
use crate::ops::split::sanitize_stem;
use crate::{Progress, SheafError};

/// Options for [`protect`].
#[derive(Debug, Clone)]
pub struct ProtectOptions {
    /// Loading and decryption options for input PDF.
    pub load: LoadOptions,
    /// User password required to open the encrypted PDF.
    pub user_password: SecretString,
    /// Optional owner password (defaults to `user_password` when `None`).
    pub owner_password: Option<SecretString>,
    /// Allow printing the document (`true` by default).
    pub allow_print: bool,
    /// Allow copying text and graphics (`true` by default).
    pub allow_copy: bool,
    /// Allow modifying contents, annotations, and forms (`true` by default).
    pub allow_modify: bool,
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
            load: LoadOptions::default(),
            user_password: SecretString::from(""),
            owner_password: None,
            allow_print: true,
            allow_copy: true,
            allow_modify: true,
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
    pub password: SecretString,
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
    /// Encryption algorithm (`"aes-256"`, `"aes-128"`, or `"none"`).
    pub algorithm: String,
    /// Key length in bits (256, 128, or 0).
    pub key_bits: u32,
    /// Decoded permission flags.
    pub permissions: EncryptionPermissionsInfo,
    /// Whether the input file was originally encrypted.
    #[serde(skip)]
    pub was_encrypted: bool,
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
///     user_password: "secret-password".to_string().into(),
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
    if opts.user_password.expose_secret().is_empty() {
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
    let loaded = load_pdf_from_bytes(&raw_bytes, src, &opts.load)?;
    let mut doc = loaded.doc;

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
        if opts.allow_modify {
            perms |= Permissions::MODIFIABLE | Permissions::ANNOTABLE;
        }

        let owner_pw = opts
            .owner_password
            .as_ref()
            .map(|s| s.expose_secret())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| opts.user_password.expose_secret());

        let mut crypt_filters = std::collections::BTreeMap::new();
        crypt_filters.insert(
            b"StdCF".to_vec(),
            std::sync::Arc::new(lopdf::encryption::crypt_filters::Aes256CryptFilter)
                as std::sync::Arc<dyn lopdf::encryption::crypt_filters::CryptFilter>,
        );

        let key = [42u8; 32];
        let state = EncryptionState::try_from(EncryptionVersion::V5 {
            encrypt_metadata: true,
            crypt_filters,
            file_encryption_key: &key,
            stream_filter: b"StdCF".to_vec(),
            string_filter: b"StdCF".to_vec(),
            owner_password: owner_pw,
            user_password: opts.user_password.expose_secret(),
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
        algorithm: "aes-256".to_string(),
        key_bits: 256,
        permissions: EncryptionPermissionsInfo {
            print: opts.allow_print,
            copy: opts.allow_copy,
            modify: opts.allow_modify,
        },
        was_encrypted: loaded.was_encrypted,
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
///     password: "secret-password".to_string().into(),
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

    // Check if the document is unencrypted
    let unenc_probe = load_pdf(src, &LoadOptions::none());
    let (mut doc, was_encrypted) = if let Ok(unenc) = unenc_probe
        && !unenc.was_encrypted
    {
        // Unencrypted input per M10 spec:
        // Exit 0, Warning "Datei ist nicht verschlüsselt", Ausgabe = Kopie
        (unenc.doc, false)
    } else {
        let loaded = load_pdf(
            src,
            &LoadOptions {
                password: Some(opts.password.clone()),
            },
        )?;
        let mut d = loaded.doc;
        d.trailer.remove(b"Encrypt");
        d.encryption_state = None;
        (d, loaded.was_encrypted)
    };

    let pages_count = doc.get_pages().len() as u32;
    if pages_count == 0 {
        return Err(SheafError::WrongPassword);
    }

    if p.is_cancelled() {
        return Err(SheafError::Cancelled);
    }
    p.report(60, 100);

    if !opts.dry_run {
        if !was_encrypted {
            fs::copy(src, &out_path)?;
        } else {
            doc.compress();
            write_atomic(&out_path, opts.force, |file| {
                let mut writer = BufWriter::new(file);
                doc.save_to(&mut writer)
                    .map_err(|e| std::io::Error::other(format!("Failed to write PDF: {e}")))?;
                writer.flush()?;
                Ok(())
            })?;
        }
    }

    p.report(100, 100);

    Ok(ProtectReport {
        action: "unlock",
        input: src.to_path_buf(),
        output: out_path,
        pages: pages_count,
        encrypted: false,
        dry_run: opts.dry_run,
        algorithm: "none".to_string(),
        key_bits: 0,
        permissions: EncryptionPermissionsInfo {
            print: true,
            copy: true,
            modify: true,
        },
        was_encrypted,
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
