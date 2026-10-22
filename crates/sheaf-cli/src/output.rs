use std::io::Write;

use serde::Serialize;
use sheaf_core::{SCHEMA_VERSION, SheafError};

use crate::cli::GlobalArgs;
use crate::exit::{EXIT_USAGE, exit_code};
use crate::progress::{CliProgress, EffectiveProgressMode};

/// Successful JSON envelope written to `stdout` when `--json` is active.
#[derive(Debug, Serialize)]
pub struct SuccessEnvelope<T: Serialize> {
    pub schema: u32,
    pub ok: bool,
    pub op: &'static str,
    pub dry_run: bool,
    pub data: T,
    pub warnings: Vec<String>,
    pub duration_ms: u64,
}

/// Error JSON payload embedded in [`ErrorEnvelope`].
#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: &'static str,
    pub exit_code: u8,
    pub message: String,
    pub hint: Option<String>,
}

/// Failed JSON envelope written to `stdout` when `--json` is active.
#[derive(Debug, Serialize)]
pub struct ErrorEnvelope<'a> {
    pub schema: u32,
    pub ok: bool,
    pub op: &'a str,
    pub error: ErrorDetail,
}

/// Failed JSON envelope when an operation failed *with* structured data (such as `sheaf verify`).
#[derive(Debug, Serialize)]
pub struct ErrorWithDataEnvelope<'a, T: Serialize> {
    pub schema: u32,
    pub ok: bool,
    pub op: &'a str,
    pub dry_run: bool,
    pub data: T,
    pub warnings: Vec<String>,
    pub duration_ms: u64,
    pub error: ErrorDetail,
}

/// Renders a successful command outcome to `stdout` (and warnings to `stderr` if applicable).
#[allow(clippy::too_many_arguments)]
pub fn render_success<T: Serialize>(
    op: &'static str,
    report: &T,
    human_summary: &str,
    warnings: Vec<String>,
    dry_run: bool,
    duration_ms: u64,
    global: &GlobalArgs,
    progress: &CliProgress,
) {
    if global.json {
        if progress.mode() == EffectiveProgressMode::Json {
            for w in &warnings {
                progress.emit_warning(w, global.quiet);
            }
        }
        let env = SuccessEnvelope {
            schema: SCHEMA_VERSION,
            ok: true,
            op,
            dry_run,
            data: report,
            warnings,
            duration_ms,
        };
        if let Ok(json_str) = serde_json::to_string(&env) {
            let _ = writeln!(std::io::stdout(), "{json_str}");
        }
    } else {
        for w in &warnings {
            progress.emit_warning(w, global.quiet);
        }
        if !global.quiet && !human_summary.is_empty() {
            let _ = writeln!(std::io::stdout(), "{human_summary}");
        }
    }
}

/// Renders a failed [`SheafError`] to `stdout` (when `--json`) or `stderr` (human mode)
/// and returns its mapped exit code.
pub fn render_error(op: &str, err: &SheafError, global: &GlobalArgs) -> u8 {
    let code_num = exit_code(err);
    if global.json {
        let env = ErrorEnvelope {
            schema: SCHEMA_VERSION,
            ok: false,
            op,
            error: ErrorDetail {
                code: err.code(),
                exit_code: code_num,
                message: err.to_string(),
                hint: err.hint(),
            },
        };
        if let Ok(json_str) = serde_json::to_string(&env) {
            let _ = writeln!(std::io::stdout(), "{json_str}");
        }
    } else {
        let mut stderr = std::io::stderr();
        let _ = writeln!(stderr, "Error: {err}");
        if let Some(hint) = err.hint() {
            let _ = writeln!(stderr, "Hint: {hint}");
        }
    }
    code_num
}

/// Renders an outcome that includes structured `data` even though an error occurred (e.g. `verify` with invalid files).
#[allow(clippy::too_many_arguments)]
pub fn render_error_with_data<T: Serialize>(
    op: &'static str,
    report: &T,
    human_summary: &str,
    warnings: Vec<String>,
    dry_run: bool,
    duration_ms: u64,
    err: &SheafError,
    global: &GlobalArgs,
    progress: &CliProgress,
) -> u8 {
    let code_num = exit_code(err);
    if global.json {
        if progress.mode() == EffectiveProgressMode::Json {
            for w in &warnings {
                progress.emit_warning(w, global.quiet);
            }
        }
        let env = ErrorWithDataEnvelope {
            schema: SCHEMA_VERSION,
            ok: false,
            op,
            dry_run,
            data: report,
            warnings,
            duration_ms,
            error: ErrorDetail {
                code: err.code(),
                exit_code: code_num,
                message: err.to_string(),
                hint: err.hint(),
            },
        };
        if let Ok(json_str) = serde_json::to_string(&env) {
            let _ = writeln!(std::io::stdout(), "{json_str}");
        }
    } else {
        for w in &warnings {
            progress.emit_warning(w, global.quiet);
        }
        if !global.quiet && !human_summary.is_empty() {
            let _ = writeln!(std::io::stdout(), "{human_summary}");
        }
        let mut stderr = std::io::stderr();
        let _ = writeln!(stderr, "Error: {err}");
        if let Some(hint) = err.hint() {
            let _ = writeln!(stderr, "Hint: {hint}");
        }
    }
    code_num
}

/// Renders a `clap::Error` when CLI parsing fails.
/// If `--json` was present in raw `argv`, emits a valid JSON error envelope on `stdout` and returns exit code 2.
pub fn render_clap_error(err: clap::Error, raw_args: &[String]) -> u8 {
    let json_requested = raw_args.iter().any(|a| a == "--json");

    if matches!(
        err.kind(),
        clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
    ) {
        if json_requested && err.kind() == clap::error::ErrorKind::DisplayVersion {
            let version_info = serde_json::json!({
                "schema": SCHEMA_VERSION,
                "version": env!("CARGO_PKG_VERSION"),
                "commit": env!("SHEAF_GIT_HASH"),
                "date": env!("SHEAF_GIT_DATE"),
                "target": env!("SHEAF_TARGET_TRIPLE"),
            });
            let _ = writeln!(std::io::stdout(), "{version_info}");
            return 0;
        }
        let _ = err.print();
        return 0;
    }

    if json_requested {
        let op = detect_op_from_args(raw_args);
        let msg = err.render().to_string();
        let first_line = msg
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("Invalid command-line arguments")
            .trim()
            .to_string();

        let env = ErrorEnvelope {
            schema: SCHEMA_VERSION,
            ok: false,
            op,
            error: ErrorDetail {
                code: "invalid_selection",
                exit_code: EXIT_USAGE,
                message: first_line,
                hint: Some("Run with '--help' for valid options and examples.".to_string()),
            },
        };
        if let Ok(json_str) = serde_json::to_string(&env) {
            let _ = writeln!(std::io::stdout(), "{json_str}");
        }
    } else {
        let _ = err.print();
    }

    EXIT_USAGE
}

fn detect_op_from_args(raw_args: &[String]) -> &str {
    for arg in raw_args.iter().skip(1) {
        match arg.as_str() {
            "merge" | "split" | "pages" | "render" | "compress" | "convert" | "watermark"
            | "protect" | "unlock" | "info" | "verify" | "capabilities" => {
                return arg.as_str();
            }
            _ => {}
        }
    }
    "cli"
}
