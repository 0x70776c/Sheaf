//! Central PDF loading pipeline for Sheaf.
//!
//! All operations in Sheaf must load PDFs exclusively via [`load_pdf`]
//! or [`load_pdf_from_bytes`]. Direct invocation of `lopdf::Document::load*`
//! is strictly forbidden by project rules and enforced via Clippy.

use std::fs;
use std::path::Path;

use lopdf::{Document, LoadOptions as LopdfLoadOptions};
use secrecy::{ExposeSecret, SecretString};

use crate::SheafError;

/// Options controlling how a PDF is loaded and decrypted.
#[derive(Debug, Clone, Default)]
pub struct LoadOptions {
    /// Optional password to decrypt the PDF if it is encrypted.
    pub password: Option<SecretString>,
}

impl LoadOptions {
    /// Creates a new [`LoadOptions`] with no password.
    pub fn none() -> Self {
        Self { password: None }
    }

    /// Creates a new [`LoadOptions`] with the given password.
    pub fn new(password: Option<SecretString>) -> Self {
        Self { password }
    }
}

/// Result of a successful [`load_pdf`] call.
pub struct LoadedPdf {
    /// The loaded and (if encrypted) decrypted in-memory document.
    pub doc: Document,
    /// Indicates whether the source document was encrypted.
    pub was_encrypted: bool,
}

/// The sole permitted entry point for loading PDFs from the file system.
///
/// # Errors
/// - [`SheafError::Io`] if the file cannot be read.
/// - [`SheafError::Encrypted`] if the file is encrypted and no password was provided.
/// - [`SheafError::WrongPassword`] if the provided password is incorrect.
/// - [`SheafError::InvalidPdf`] if the file is corrupted or not a valid PDF.
pub fn load_pdf(path: &Path, opts: &LoadOptions) -> Result<LoadedPdf, SheafError> {
    if !path.exists() {
        return Err(SheafError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found: {}", path.display()),
        )));
    }

    let raw_bytes = fs::read(path)?;
    load_pdf_from_bytes(&raw_bytes, path, opts)
}

/// Loads a PDF from an in-memory byte slice using the central loading rules.
#[allow(clippy::disallowed_methods)]
pub fn load_pdf_from_bytes(
    raw_bytes: &[u8],
    path_hint: &Path,
    opts: &LoadOptions,
) -> Result<LoadedPdf, SheafError> {
    // 1. Try loading without password first to check if the PDF is unencrypted.
    let unencrypted_probe = Document::load_mem(raw_bytes);

    match unencrypted_probe {
        Ok(doc) => {
            let is_enc = doc.is_encrypted()
                || doc.trailer.get(b"Encrypt").is_ok()
                || raw_bytes.windows(8).any(|w| w == b"/Encrypt");

            if !is_enc {
                // Not encrypted: ignore any supplied password per spec.
                return Ok(LoadedPdf {
                    doc,
                    was_encrypted: false,
                });
            }

            // Encrypted: if no password was provided, return Encrypted.
            let pw = match &opts.password {
                Some(p) => p,
                None => return Err(SheafError::Encrypted),
            };

            // Attempt decrypting with the password.
            decrypt_doc(raw_bytes, path_hint, pw.expose_secret())
        }
        Err(lopdf::Error::Decryption(_) | lopdf::Error::InvalidPassword) => {
            // Document required password to even parse streams.
            let pw = match &opts.password {
                Some(p) => p,
                None => return Err(SheafError::Encrypted),
            };
            decrypt_doc(raw_bytes, path_hint, pw.expose_secret())
        }
        Err(e) => {
            // Check if failure is due to encryption markers
            if raw_bytes.windows(8).any(|w| w == b"/Encrypt") {
                if let Some(pw) = &opts.password {
                    return decrypt_doc(raw_bytes, path_hint, pw.expose_secret());
                }
                return Err(SheafError::Encrypted);
            }
            Err(SheafError::from_lopdf_with_path(e, path_hint))
        }
    }
}

#[allow(clippy::disallowed_methods)]
fn decrypt_doc(
    raw_bytes: &[u8],
    path_hint: &Path,
    password: &str,
) -> Result<LoadedPdf, SheafError> {
    let lopdf_opts = LopdfLoadOptions::with_password(password);
    let mut doc = match Document::load_mem_with_options(raw_bytes, lopdf_opts) {
        Ok(d) => d,
        Err(lopdf::Error::Decryption(_) | lopdf::Error::InvalidPassword) => {
            return Err(SheafError::WrongPassword);
        }
        Err(e) => return Err(SheafError::from_lopdf_with_path(e, path_hint)),
    };

    if doc.is_encrypted() && doc.decrypt(password).is_err() {
        return Err(SheafError::WrongPassword);
    }

    Ok(LoadedPdf {
        doc,
        was_encrypted: true,
    })
}
