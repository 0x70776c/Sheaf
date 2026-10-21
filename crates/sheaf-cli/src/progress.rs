use std::io::{IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use sheaf_core::Progress;

use crate::cli::ProgressMode;

#[derive(Serialize)]
struct ProgressEvent<'a> {
    event: &'static str,
    op: &'a str,
    done: u64,
    total: u64,
}

#[derive(Serialize)]
struct WarningEvent<'a> {
    event: &'static str,
    message: &'a str,
}

/// CLI implementation of [`Progress`] supporting `none`, `human` (TTY), and `json` (NDJSON on stderr).
pub struct CliProgress {
    op: &'static str,
    mode: EffectiveProgressMode,
    cancelled: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectiveProgressMode {
    None,
    Human,
    Json,
}

impl CliProgress {
    pub fn new(
        op: &'static str,
        requested: Option<ProgressMode>,
        quiet: bool,
        cancelled: Arc<AtomicBool>,
    ) -> Self {
        let mode = match requested {
            Some(ProgressMode::None) => EffectiveProgressMode::None,
            Some(ProgressMode::Human) => {
                if quiet {
                    EffectiveProgressMode::None
                } else {
                    EffectiveProgressMode::Human
                }
            }
            Some(ProgressMode::Json) => EffectiveProgressMode::Json,
            None => {
                if !quiet && std::io::stderr().is_terminal() {
                    EffectiveProgressMode::Human
                } else {
                    EffectiveProgressMode::None
                }
            }
        };

        Self {
            op,
            mode,
            cancelled,
        }
    }

    pub fn mode(&self) -> EffectiveProgressMode {
        self.mode
    }

    /// Emits a warning event to stderr according to the active progress/output mode.
    pub fn emit_warning(&self, message: &str, quiet: bool) {
        match self.mode {
            EffectiveProgressMode::Json => {
                let evt = WarningEvent {
                    event: "warning",
                    message,
                };
                if let Ok(line) = serde_json::to_string(&evt) {
                    let _ = writeln!(std::io::stderr(), "{line}");
                }
            }
            EffectiveProgressMode::Human | EffectiveProgressMode::None => {
                if !quiet {
                    let _ = writeln!(std::io::stderr(), "Warnung: {message}");
                }
            }
        }
    }
}

impl Progress for CliProgress {
    fn report(&self, done: u64, total: u64) {
        match self.mode {
            EffectiveProgressMode::None => {}
            EffectiveProgressMode::Human => {
                let mut stderr = std::io::stderr();
                if done >= total && total > 0 {
                    let _ = write!(stderr, "\r\x1b[2K");
                } else {
                    let _ = write!(stderr, "\r[{}] {done}/{total}...", self.op);
                }
                let _ = stderr.flush();
            }
            EffectiveProgressMode::Json => {
                let evt = ProgressEvent {
                    event: "progress",
                    op: self.op,
                    done,
                    total,
                };
                if let Ok(line) = serde_json::to_string(&evt) {
                    let _ = writeln!(std::io::stderr(), "{line}");
                }
            }
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}
