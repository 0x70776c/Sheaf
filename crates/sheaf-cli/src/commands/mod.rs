use sheaf_core::SheafError;

use crate::cli::GlobalArgs;
use crate::progress::CliProgress;

pub mod capabilities;
pub mod compress;
pub mod info;
pub mod merge;
pub mod pages;
pub mod render;
pub mod split;
pub mod verify;

/// Execution context passed to every CLI command.
pub struct Ctx {
    pub global: GlobalArgs,
    pub progress: CliProgress,
}

/// Successful or data-carrying outcome of a CLI command.
pub struct Outcome<R> {
    pub report: R,
    pub warnings: Vec<String>,
    pub dry_run: bool,
    /// Optional non-zero error when the command produced a full report (`report`)
    /// that should still be rendered alongside the failure (e.g. `verify`).
    pub failure: Option<SheafError>,
}

/// Trait implemented by all CLI subcommands.
pub trait Command {
    type Report: serde::Serialize;

    /// Stable machine name of the command (`"merge"`, `"split"`, `"info"`, `"verify"`, `"capabilities"`).
    fn name(&self) -> &'static str;

    /// Executes the command and returns an [`Outcome`] or [`SheafError`].
    fn run(&self, ctx: &Ctx) -> Result<Outcome<Self::Report>, SheafError>;

    /// Formats the report for human-readable output on `stdout`.
    fn format_human(&self, report: &Self::Report, dry_run: bool) -> String;
}
