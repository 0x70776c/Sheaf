use std::path::{Path, PathBuf};
use thiserror::Error;

/// Error type for all Sheaf PDF operations.
///
/// Messages are formatted in clear, user-friendly English, while [`SheafError::code`]
/// provides a stable identifier for JSON and machine consumers.
#[derive(Debug, Error)]
pub enum SheafError {
    /// File system read/write error.
    #[error("File I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The input file is not a valid or supported PDF document.
    #[error("Invalid or corrupted PDF file: {0}")]
    InvalidPdf(String),

    /// The PDF document is password-protected and requires unlocking.
    #[error("File is password-protected")]
    Encrypted,

    /// The provided password is incorrect.
    #[error("Incorrect password")]
    WrongPassword,

    /// A requested 1-based page number exceeds the document's page count or is 0.
    #[error("Page {page} is out of range (document has {total} pages)")]
    PageOutOfRange {
        /// The requested 1-based page number.
        page: u32,
        /// Total number of pages in the document.
        total: u32,
    },

    /// No pages or input files were selected for the operation.
    #[error("No pages or files selected")]
    EmptySelection,

    /// Invalid page range syntax or invalid selection argument (e.g. `5-3`, `abc`, `N=0`).
    #[error("Invalid selection: {0}")]
    InvalidSelection(String),

    /// Target output file already exists and `--force` was not set.
    #[error("Output file already exists: {}", .0.display())]
    OutputExists(PathBuf),

    /// The PDF uses a feature that is not supported or lacks required elements (e.g. outlines).
    #[error("Unsupported feature: {0}")]
    Unsupported(String),

    /// The operation was cancelled by the user.
    #[error("Operation cancelled")]
    Cancelled,
}

impl SheafError {
    /// Returns the stable machine-readable error code for this error variant.
    ///
    /// # Examples
    ///
    /// ```
    /// use sheaf_core::SheafError;
    ///
    /// assert_eq!(SheafError::Encrypted.code(), "encrypted");
    /// assert_eq!(SheafError::EmptySelection.code(), "empty_selection");
    /// ```
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "io",
            Self::InvalidPdf(_) => "invalid_pdf",
            Self::Encrypted => "encrypted",
            Self::WrongPassword => "wrong_password",
            Self::PageOutOfRange { .. } => "page_out_of_range",
            Self::EmptySelection => "empty_selection",
            Self::InvalidSelection(_) => "invalid_selection",
            Self::OutputExists(_) => "output_exists",
            Self::Unsupported(_) => "unsupported",
            Self::Cancelled => "cancelled",
        }
    }

    /// Returns an optional user-facing hint explaining how to resolve the error.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use sheaf_core::SheafError;
    ///
    /// let err = SheafError::OutputExists(PathBuf::from("out.pdf"));
    /// assert!(err.hint().is_some());
    /// ```
    pub fn hint(&self) -> Option<String> {
        match self {
            Self::Io(_) => Some("Check the file path and permissions.".to_string()),
            Self::InvalidPdf(_) => Some("Verify the file with 'sheaf verify <file>'.".to_string()),
            Self::Encrypted => Some("Unlock or remove the password protection first.".to_string()),
            Self::WrongPassword => Some("Please check the provided password.".to_string()),
            Self::PageOutOfRange { total, .. } => {
                if *total > 0 {
                    Some(format!("Valid page range: 1-{total}."))
                } else {
                    Some("The document contains no pages.".to_string())
                }
            }
            Self::EmptySelection => {
                Some("Provide at least one input file or page range.".to_string())
            }
            Self::InvalidSelection(_) => {
                Some("Expected page range format: e.g. '1-3,5,8-'.".to_string())
            }
            Self::OutputExists(_) => {
                Some("Use '--force' to overwrite existing output files.".to_string())
            }
            Self::Unsupported(msg) => Some(msg.clone()),
            Self::Cancelled => None,
        }
    }

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
    pub fn from_lopdf_with_path(err: lopdf::Error, path: &Path) -> Self {
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
    out: &Path,
) -> Result<(), SheafError> {
    let out_canonical = std::fs::canonicalize(out).ok();
    for input in inputs {
        if input == out {
            return Err(SheafError::InvalidSelection(
                "Output file must not overwrite an input file".to_string(),
            ));
        }
        if let (Some(in_can), Some(out_can)) = (std::fs::canonicalize(input).ok(), &out_canonical)
            && &in_can == out_can
        {
            return Err(SheafError::InvalidSelection(
                "Output file must not overwrite an input file".to_string(),
            ));
        }
    }
    Ok(())
}
