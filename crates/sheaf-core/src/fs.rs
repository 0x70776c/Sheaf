//! Atomic file system operations for Sheaf.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::SheafError;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Checks if `path` already exists when `force` is `false`.
///
/// # Errors
///
/// Returns [`SheafError::OutputExists`] if `path` exists and `!force`.
pub fn check_output_path(path: &Path, force: bool) -> Result<(), SheafError> {
    if !force && path.exists() {
        return Err(SheafError::OutputExists(path.to_path_buf()));
    }
    Ok(())
}

/// Writes to `path` atomically by first creating a temporary file in the same directory
/// and renaming it to `path` only after `f` succeeds.
///
/// If `f` returns an error, the temporary file is removed and `path` remains untouched.
///
/// # Errors
///
/// Returns:
/// - [`SheafError::OutputExists`] if `path` already exists and `force` is `false`.
/// - [`SheafError::Io`] if creating the parent directory, temporary file, or renaming fails.
///
/// # Examples
///
/// ```no_run
/// use std::io::Write;
/// use std::path::Path;
/// use sheaf_core::fs::write_atomic;
///
/// write_atomic(Path::new("output.pdf"), false, |file| {
///     file.write_all(b"%PDF-1.5")?;
///     Ok(())
/// }).unwrap();
/// ```
pub fn write_atomic(
    path: &Path,
    force: bool,
    f: impl FnOnce(&mut File) -> Result<(), SheafError>,
) -> Result<(), SheafError> {
    check_output_path(path, force)?;

    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    std::fs::create_dir_all(parent)?;

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("output.pdf");
    let pid = std::process::id();

    let (mut temp_file, temp_path) = loop {
        let seq = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(".{file_name}.sheaf-tmp-{pid}-{seq}"));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => break (file, candidate),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(SheafError::Io(e)),
        }
    };

    match f(&mut temp_file) {
        Ok(()) => {
            if let Err(e) = temp_file.flush() {
                let _ = std::fs::remove_file(&temp_path);
                return Err(SheafError::Io(e));
            }
            if let Err(e) = temp_file.sync_all() {
                let _ = std::fs::remove_file(&temp_path);
                return Err(SheafError::Io(e));
            }
            drop(temp_file);

            // Re-check before rename in case `force` is false and a file appeared concurrently
            if !force && path.exists() {
                let _ = std::fs::remove_file(&temp_path);
                return Err(SheafError::OutputExists(path.to_path_buf()));
            }

            if let Err(e) = std::fs::rename(&temp_path, path) {
                let _ = std::fs::remove_file(&temp_path);
                return Err(SheafError::Io(e));
            }
            Ok(())
        }
        Err(err) => {
            drop(temp_file);
            let _ = std::fs::remove_file(&temp_path);
            Err(err)
        }
    }
}
