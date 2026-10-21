use std::path::PathBuf;

use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};

/// Progress reporting mode on `stderr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ProgressMode {
    /// Disable progress output.
    None,
    /// Human-readable status line on stderr.
    Human,
    /// Newline-delimited JSON (NDJSON) events on stderr.
    Json,
}

/// Global flags available on every `sheaf` command.
#[derive(Debug, Clone, Default, Args)]
pub struct GlobalArgs {
    /// Output a single JSON envelope object on stdout
    #[arg(long, global = true)]
    pub json: bool,

    /// Validate inputs and compute execution plan without writing any files
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Overwrite existing output files
    #[arg(long, global = true)]
    pub force: bool,

    /// Suppress human-readable output except errors
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Emit verbose debug logs on stderr
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Progress output mode on stderr (default: human on TTY, otherwise none)
    #[arg(long, value_enum, global = true)]
    pub progress: Option<ProgressMode>,

    /// Disable colored console output (also respects NO_COLOR)
    #[arg(long, global = true)]
    pub no_color: bool,
}

#[derive(Debug, Parser)]
#[command(
    name = "sheaf",
    version,
    about = "Fast, 100% offline PDF toolkit for macOS",
    after_help = "Examples:\n  sheaf merge a.pdf b.pdf -o combined.pdf\n  sheaf split report.pdf --ranges \"1-3,8-\" -o out/\n  sheaf pages rotate report.pdf --pages \"1-3\" --degrees 90 -o rotated.pdf\n  sheaf info report.pdf --json"
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Merge multiple PDF files into a single output PDF
    #[command(
        after_help = "Examples:\n  sheaf merge a.pdf b.pdf -o out.pdf\n  sheaf merge part1.pdf part2.pdf -o combined.pdf --force --json"
    )]
    Merge(MergeArgs),

    /// Split a PDF file by page ranges, fixed chunks, or bookmark levels
    #[command(
        after_help = "Examples:\n  sheaf split report.pdf --ranges \"1-3,5,8-\" -o out/\n  sheaf split report.pdf --every 2 -o out/\n  sheaf split report.pdf --bookmarks-level 1 -o out/ --json"
    )]
    Split(SplitArgs),

    /// Page-level operations: rotate, delete, reorder, or extract pages
    #[command(
        after_help = "Examples:\n  sheaf pages rotate report.pdf --pages \"1-3\" --degrees 90 -o rotated.pdf\n  sheaf pages delete report.pdf --pages \"2,5-7\" -o cleaned.pdf\n  sheaf pages reorder report.pdf --order \"3,1,2,4-\" -o reordered.pdf\n  sheaf pages extract report.pdf --pages \"1,4-6\" -o subset.pdf"
    )]
    Pages(PagesArgs),

    /// Inspect structural properties and metadata of PDF files
    #[command(after_help = "Examples:\n  sheaf info document.pdf\n  sheaf info a.pdf b.pdf --json")]
    Info(InfoArgs),

    /// Verify PDF files for structural integrity
    #[command(
        after_help = "Examples:\n  sheaf verify document.pdf\n  sheaf verify *.pdf --deep --json"
    )]
    Verify(VerifyArgs),

    /// Describe supported commands, flags, exit codes, and external tools
    #[command(after_help = "Examples:\n  sheaf capabilities --json\n  sheaf capabilities")]
    Capabilities(CapabilitiesArgs),
}

#[derive(Debug, Clone, Args)]
pub struct MergeArgs {
    /// Input PDF files in the desired merge order
    #[arg(required = true)]
    pub inputs: Vec<PathBuf>,

    /// Target output PDF file
    #[arg(short, long)]
    pub out: PathBuf,
}

#[derive(Debug, Clone, Args)]
#[command(group(
    ArgGroup::new("split_mode")
        .required(true)
        .args(["ranges", "every", "bookmarks_level"])
))]
pub struct SplitArgs {
    /// Input PDF file
    pub input: PathBuf,

    /// Page ranges to split (e.g. "1-3,5,8-")
    #[arg(long)]
    pub ranges: Option<String>,

    /// Split every N pages
    #[arg(long)]
    pub every: Option<u32>,

    /// Split by bookmarks at hierarchy level N (1 = top level)
    #[arg(long)]
    pub bookmarks_level: Option<u8>,

    /// Combine selected page ranges into a single PDF file (only valid with --ranges)
    #[arg(long, requires = "ranges", conflicts_with_all = ["every", "bookmarks_level"])]
    pub combine: bool,

    /// Output directory for generated PDF files (defaults to input file directory)
    #[arg(short, long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct PagesArgs {
    #[command(subcommand)]
    pub action: PagesSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum PagesSubcommand {
    /// Rotate selected pages (or all pages) additively by 90, 180, or 270 degrees
    #[command(
        after_help = "Examples:\n  sheaf pages rotate input.pdf --degrees 90 -o rotated.pdf\n  sheaf pages rotate input.pdf --pages \"1,3-5\" --degrees 180 -o rotated.pdf --json"
    )]
    Rotate(PagesRotateArgs),

    /// Delete selected pages from a PDF file
    #[command(
        after_help = "Examples:\n  sheaf pages delete input.pdf --pages \"2,4-6\" -o out.pdf\n  sheaf pages delete input.pdf --pages \"1\" -o without_cover.pdf --json"
    )]
    Delete(PagesDeleteArgs),

    /// Reorder pages according to a page range sequence
    #[command(
        after_help = "Examples:\n  sheaf pages reorder input.pdf --order \"3,1,2,4-\" -o reordered.pdf\n  sheaf pages reorder input.pdf --order \"2,1\" -o swapped.pdf --json"
    )]
    Reorder(PagesReorderArgs),

    /// Extract selected pages into a new PDF file
    #[command(
        after_help = "Examples:\n  sheaf pages extract input.pdf --pages \"1-3,8\" -o extracted.pdf\n  sheaf pages extract input.pdf --pages \"5-\" -o tail.pdf --json"
    )]
    Extract(PagesExtractArgs),
}

#[derive(Debug, Clone, Args)]
pub struct PagesRotateArgs {
    /// Input PDF file
    pub input: PathBuf,

    /// Page ranges to rotate (e.g. "1-3,5"; omit to rotate all pages)
    #[arg(long)]
    pub pages: Option<String>,

    /// Clockwise rotation angle in degrees (90, 180, 270, or -90)
    #[arg(long, allow_hyphen_values = true)]
    pub degrees: i32,

    /// Target output PDF file (defaults to {stem}_rotated.pdf)
    #[arg(short, long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct PagesDeleteArgs {
    /// Input PDF file
    pub input: PathBuf,

    /// Page ranges to delete (e.g. "2,5-7")
    #[arg(long)]
    pub pages: String,

    /// Target output PDF file (defaults to {stem}_deleted.pdf)
    #[arg(short, long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct PagesReorderArgs {
    /// Input PDF file
    pub input: PathBuf,

    /// New page order specification (e.g. "3,1,2,4-")
    #[arg(long)]
    pub order: String,

    /// Target output PDF file (defaults to {stem}_reordered.pdf)
    #[arg(short, long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct PagesExtractArgs {
    /// Input PDF file
    pub input: PathBuf,

    /// Page ranges to extract into a new PDF (e.g. "1-3,5")
    #[arg(long)]
    pub pages: String,

    /// Target output PDF file (defaults to {stem}_extracted.pdf)
    #[arg(short, long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct InfoArgs {
    /// PDF files to inspect
    #[arg(required = true)]
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct VerifyArgs {
    /// PDF files to verify
    #[arg(required = true)]
    pub files: Vec<PathBuf>,

    /// Also run qpdf --check if installed
    #[arg(long)]
    pub deep: bool,
}

#[derive(Debug, Clone, Default, Args)]
pub struct CapabilitiesArgs {}
