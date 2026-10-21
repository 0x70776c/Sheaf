use std::path::PathBuf;
use thiserror::Error;

/// Error type for all Sheaf PDF operations.
///
/// Messages are formatted to be user-friendly and understandable without
/// exposing low-level internal error codes.
#[derive(Debug, Error)]
pub enum SheafError {
    /// File system read/write error.
    #[error("Dateifehler: {0}")]
    Io(#[from] std::io::Error),

    /// The input file is not a valid or supported PDF document.
    #[error("Ungültige oder beschädigte PDF-Datei: {0}")]
    InvalidPdf(String),

    /// The PDF document is password-protected and requires unlocking.
    #[error("Datei ist passwortgeschützt")]
    Encrypted,

    /// The provided password is incorrect.
    #[error("Das eingegebene Passwort ist falsch")]
    WrongPassword,

    /// A requested 1-based page number exceeds the document's page count or is 0.
    #[error("Seite {page} liegt außerhalb des gültigen Bereichs (1–{total})")]
    PageOutOfRange {
        /// The requested 1-based page number.
        page: u32,
        /// Total number of pages in the document.
        total: u32,
    },

    /// No pages or input files were selected for the operation.
    #[error("Keine Seiten oder Dateien ausgewählt")]
    EmptySelection,

    /// The PDF uses a feature that is not supported.
    #[error("Nicht unterstützte Funktion: {0}")]
    Unsupported(String),

    /// The operation was cancelled by the user.
    #[error("Vorgang abgebrochen")]
    Cancelled,
}

impl SheafError {
    /// Maps a [`lopdf::Error`] into a user-friendly [`SheafError`].
    pub fn from_lopdf(err: lopdf::Error) -> Self {
        match err {
            lopdf::Error::IO(io_err) => Self::Io(io_err),
            lopdf::Error::InvalidPassword => Self::WrongPassword,
            lopdf::Error::Decryption(_) | lopdf::Error::AlreadyEncrypted => Self::Encrypted,
            lopdf::Error::UnsupportedSecurityHandler(_) | lopdf::Error::Unimplemented(_) => {
                Self::Unsupported(err.to_string())
            }
            _ => Self::InvalidPdf(err.to_string()),
        }
    }

    /// Maps a [`lopdf::Error`] while attaching the file path for context.
    pub fn from_lopdf_with_path(err: lopdf::Error, path: &std::path::Path) -> Self {
        let display = path.display();
        match err {
            lopdf::Error::IO(io_err) => Self::Io(io_err),
            lopdf::Error::InvalidPassword => Self::WrongPassword,
            lopdf::Error::Decryption(_) | lopdf::Error::AlreadyEncrypted => Self::Encrypted,
            lopdf::Error::UnsupportedSecurityHandler(_) | lopdf::Error::Unimplemented(_) => {
                Self::Unsupported(format!("{display}: {err}"))
            }
            _ => Self::InvalidPdf(format!("{display}: {err}")),
        }
    }
}

impl From<lopdf::Error> for SheafError {
    fn from(err: lopdf::Error) -> Self {
        Self::from_lopdf(err)
    }
}

/// Checks if a target output path matches any of the input paths to ensure original files are never overwritten.
pub(crate) fn ensure_not_overwriting_inputs(
    inputs: &[PathBuf],
    out: &std::path::Path,
) -> Result<(), SheafError> {
    let out_canonical = std::fs::canonicalize(out).ok();
    for input in inputs {
        if input == out {
            return Err(SheafError::InvalidPdf(
                "Ausgabedatei darf die Originaldatei nicht überschreiben".to_string(),
            ));
        }
        if let (Some(in_can), Some(out_can)) = (std::fs::canonicalize(input).ok(), &out_canonical)
            && &in_can == out_can
        {
            return Err(SheafError::InvalidPdf(
                "Ausgabedatei darf die Originaldatei nicht überschreiben".to_string(),
            ));
        }
    }
    Ok(())
}
