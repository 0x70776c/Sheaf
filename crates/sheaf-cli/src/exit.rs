use sheaf_core::SheafError;

/// Exit code for successful execution.
pub const EXIT_OK: u8 = 0;
/// Exit code for unexpected internal errors.
#[allow(dead_code)]
pub const EXIT_INTERNAL: u8 = 1;
/// Exit code for invalid arguments, usage errors, or invalid/empty selection.
pub const EXIT_USAGE: u8 = 2;
/// Exit code for corrupted or invalid PDF documents.
pub const EXIT_INVALID_PDF: u8 = 3;
/// Exit code for encrypted PDFs or wrong passwords.
pub const EXIT_ENCRYPTED: u8 = 4;
/// Exit code when a requested page number is out of range.
pub const EXIT_PAGE_OUT_OF_RANGE: u8 = 5;
/// Exit code for file system I/O errors.
pub const EXIT_IO: u8 = 6;
/// Exit code when the target output file already exists without `--force`.
pub const EXIT_OUTPUT_EXISTS: u8 = 7;
/// Exit code for unsupported PDF features or operations.
pub const EXIT_UNSUPPORTED: u8 = 8;
/// Exit code when an operation was cancelled (Ctrl+C).
pub const EXIT_CANCELLED: u8 = 130;

/// Maps every [`SheafError`] variant exhaustively to its stable CLI exit code.
pub fn exit_code(err: &SheafError) -> u8 {
    match err {
        SheafError::Io(_) => EXIT_IO,
        SheafError::InvalidPdf(_) => EXIT_INVALID_PDF,
        SheafError::Encrypted | SheafError::WrongPassword => EXIT_ENCRYPTED,
        SheafError::PageOutOfRange { .. } => EXIT_PAGE_OUT_OF_RANGE,
        SheafError::EmptySelection | SheafError::InvalidSelection(_) => EXIT_USAGE,
        SheafError::OutputExists(_) => EXIT_OUTPUT_EXISTS,
        SheafError::Unsupported(_) => EXIT_UNSUPPORTED,
        SheafError::Cancelled => EXIT_CANCELLED,
    }
}
