use std::path::PathBuf;

use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};

use crate::password::{NewPasswordArgs, OpenPasswordArgs};

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

    /// Hidden trigger to test the panic hook
    #[arg(long, hide = true, global = true)]
    pub test_panic: bool,
}

#[derive(Debug, Parser)]
#[command(
    name = "sheaf",
    version = concat!(env!("CARGO_PKG_VERSION"), " (commit ", env!("SHEAF_GIT_HASH"), " ", env!("SHEAF_GIT_DATE"), ")"),
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

    /// Render PDF pages to PNG images or thumbnails
    #[command(
        after_help = "Examples:\n  sheaf render report.pdf --page 1 --dpi 150 -o page1.png\n  sheaf render report.pdf --pages \"1-20\" --width 160 -o thumbs/ --json"
    )]
    Render(RenderArgs),

    /// Compress a PDF file by optimizing streams, deduplicating objects, and resampling images
    #[command(
        after_help = "Examples:\n  sheaf compress report.pdf --preset medium -o report_compressed.pdf\n  sheaf compress scan.pdf --preset high --grayscale -o scan_small.pdf --json"
    )]
    Compress(CompressArgs),

    /// Convert between PDF, images (PNG/JPG), and Markdown (.md)
    #[command(
        after_help = "Examples:\n  sheaf convert to-images report.pdf --format png --dpi 150 -o images/\n  sheaf convert from-images page1.png page2.jpg --page-size a4 -o combined.pdf\n  sheaf convert to-markdown report.pdf -o report.md\n  sheaf convert from-markdown notes.md --page-size a4 -o notes.pdf --json"
    )]
    Convert(ConvertArgs),

    /// Add a customizable text watermark to PDF pages
    #[command(
        after_help = "Examples:\n  sheaf watermark report.pdf --text \"CONFIDENTIAL\" -o watermarked.pdf\n  sheaf watermark draft.pdf --text \"DRAFT\" --position bottom-right --opacity 0.35 --font-size 36 -o out.pdf --json"
    )]
    Watermark(WatermarkArgs),

    /// Stamp customizable page numbers onto PDF pages
    #[command(
        after_help = "Examples:\n  sheaf number report.pdf -o numbered.pdf\n  sheaf number thesis.pdf --format \"Page {n} of {total}\" --position bottom-right --start 1 --pages \"2-\" -o thesis_numbered.pdf --json"
    )]
    Number(NumberArgs),

    /// Password-protect (encrypt) a PDF file
    #[command(
        long_about = "Password-protect (encrypt) a PDF file.\n\nHinweis: --no-print, --no-copy, --no-modify sind Hinweise an den PDF-Viewer. Manche Programme ignorieren sie. Echter Schutz entsteht nur durch das User-Passwort.",
        after_help = "Examples:\n  sheaf protect report.pdf --new-password-env PDF_PASS -o protected.pdf\n  sheaf protect contract.pdf --new-password-file /path/to/pw --new-owner-password-env OWNER_PASS --no-copy -o locked.pdf --json\n\nHinweis:\n  --no-print, --no-copy, --no-modify sind Hinweise an den PDF-Viewer. Manche Programme ignorieren sie. Echter Schutz entsteht nur durch das User-Passwort."
    )]
    Protect(ProtectArgs),

    /// Remove password protection (decrypt) from an encrypted PDF file
    #[command(
        after_help = "Examples:\n  sheaf unlock protected.pdf --password-env PDF_PASS -o unlocked.pdf\n  sheaf unlock encrypted.pdf --password-file /path/to/pw -o plain.pdf --json"
    )]
    Unlock(UnlockArgs),

    /// Inspect, modify, or strip document metadata and XMP packets
    Meta(MetaArgs),

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

    /// Generate shell completion scripts for bash, zsh, fish, elvish, or powershell
    #[command(
        after_help = "Examples:\n  sheaf completions bash > ~/.local/share/bash-completion/completions/sheaf\n  sheaf completions zsh > ~/.zsh/completion/_sheaf\n  sheaf completions fish > ~/.config/fish/completions/sheaf.fish\n  sheaf completions zsh --json"
    )]
    Completions(CompletionsArgs),

    /// Generate Markdown documentation for all commands (internal)
    #[command(hide = true)]
    Docs(DocsArgs),
}

#[derive(Debug, Clone, Args)]
pub struct MergeArgs {
    /// Input PDF files in the desired merge order
    #[arg(required = true)]
    pub inputs: Vec<PathBuf>,

    #[command(flatten)]
    pub password_opts: OpenPasswordArgs,

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

    #[command(flatten)]
    pub password_opts: OpenPasswordArgs,

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

/// Value parser for `--degrees`.
///
/// Parses any integer so that semantic validation stays in `sheaf-core`
/// (stable error code `invalid_selection`), but advertises the supported
/// values for `--help`, shell completion and `sheaf capabilities`.
#[derive(Debug, Clone, Copy)]
pub struct RotationDegreesParser;

impl clap::builder::TypedValueParser for RotationDegreesParser {
    type Value = i32;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        clap::value_parser!(i32).parse_ref(cmd, arg, value)
    }

    fn possible_values(
        &self,
    ) -> Option<Box<dyn Iterator<Item = clap::builder::PossibleValue> + '_>> {
        Some(Box::new(
            ["90", "180", "270", "-90"]
                .into_iter()
                .map(clap::builder::PossibleValue::new),
        ))
    }
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

    #[command(flatten)]
    pub password_opts: OpenPasswordArgs,

    /// Page ranges to rotate (e.g. "1-3,5"; omit to rotate all pages)
    #[arg(long)]
    pub pages: Option<String>,

    /// Clockwise rotation angle in degrees (90, 180, 270, or -90)
    #[arg(
        long,
        allow_hyphen_values = true,
        value_parser = RotationDegreesParser
    )]
    pub degrees: i32,

    /// Target output PDF file (defaults to {stem}_rotated.pdf)
    #[arg(short, long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct PagesDeleteArgs {
    /// Input PDF file
    pub input: PathBuf,

    #[command(flatten)]
    pub password_opts: OpenPasswordArgs,

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

    #[command(flatten)]
    pub password_opts: OpenPasswordArgs,

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

    #[command(flatten)]
    pub password_opts: OpenPasswordArgs,

    /// Page ranges to extract into a new PDF (e.g. "1-3,5")
    #[arg(long)]
    pub pages: String,

    /// Target output PDF file (defaults to {stem}_extracted.pdf)
    #[arg(short, long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct RenderArgs {
    /// Input PDF file
    pub input: PathBuf,

    #[command(flatten)]
    pub password_opts: OpenPasswordArgs,

    /// Single 1-based page number to render
    #[arg(long, conflicts_with = "pages")]
    pub page: Option<u32>,

    /// Page ranges to render (e.g. "1-3,5,8-"; omit both --page and --pages to render all pages)
    #[arg(long, conflicts_with = "page")]
    pub pages: Option<String>,

    /// Target resolution in dots per inch (default: 150 DPI if --width is not set)
    #[arg(long, conflicts_with = "width")]
    pub dpi: Option<u32>,

    /// Target width in pixels (aspect ratio preserved; uses on-disk thumbnail cache by default)
    #[arg(long, conflicts_with = "dpi")]
    pub width: Option<u32>,

    /// Disable the on-disk thumbnail cache when rendering by --width
    #[arg(long)]
    pub no_cache: bool,

    /// Output PNG file (when rendering a single page) or output directory
    #[arg(short, long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct CompressArgs {
    /// Input PDF file
    pub input: PathBuf,

    #[command(flatten)]
    pub password_opts: OpenPasswordArgs,

    /// Compression preset: low (220 DPI, JPEG 85), medium (150 DPI, JPEG 70), or high (100 DPI, JPEG 55)
    #[arg(long, default_value = "medium")]
    pub preset: String,

    /// Convert embedded images to grayscale
    #[arg(long)]
    pub grayscale: bool,

    /// Target output PDF file (defaults to {stem}_compressed.pdf)
    #[arg(short, long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct ConvertArgs {
    #[command(subcommand)]
    pub action: ConvertSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ConvertSubcommand {
    /// Export PDF pages as PNG or JPG images
    #[command(
        after_help = "Examples:\n  sheaf convert to-images report.pdf --format png --dpi 150 -o images/\n  sheaf convert to-images report.pdf --format jpg --dpi 300 --quality 90 --pages \"1-5\" -o jpg_out/ --json"
    )]
    ToImages(ConvertToImagesArgs),

    /// Combine PNG/JPG images into a multi-page PDF
    #[command(
        after_help = "Examples:\n  sheaf convert from-images scan1.png scan2.png -o scans.pdf\n  sheaf convert from-images photo1.jpg photo2.jpg --page-size a4 --margin 36 -o album.pdf --json"
    )]
    FromImages(ConvertFromImagesArgs),

    /// Extract structured Markdown (.md) from a PDF document
    #[command(
        after_help = "Examples:\n  sheaf convert to-markdown report.pdf -o report.md\n  sheaf convert to-markdown book.pdf --pages \"1-10\" -o chapter1.md --json"
    )]
    ToMarkdown(ConvertToMarkdownArgs),

    /// Render a Markdown (.md) document into a multi-page vector PDF
    #[command(
        after_help = "Examples:\n  sheaf convert from-markdown README.md -o README.pdf\n  sheaf convert from-markdown notes.md --page-size letter --margin 54 -o notes.pdf --json"
    )]
    FromMarkdown(ConvertFromMarkdownArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ConvertToImagesArgs {
    /// Input PDF file
    pub input: PathBuf,

    #[command(flatten)]
    pub password_opts: OpenPasswordArgs,

    /// Output image format: png or jpg
    #[arg(long, default_value = "png")]
    pub format: String,

    /// Target resolution in dots per inch (18..=1200)
    #[arg(long, default_value_t = 150)]
    pub dpi: u32,

    /// JPEG quality (1..=100; used when --format is jpg)
    #[arg(long, default_value_t = 85)]
    pub quality: u8,

    /// Optional 1-based page ranges to export (omit to export all pages)
    #[arg(long)]
    pub pages: Option<String>,

    /// Output directory (defaults to {stem}_images/)
    #[arg(short, long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct ConvertFromImagesArgs {
    /// Input image files (PNG, JPG/JPEG) in page order
    #[arg(required = true)]
    pub inputs: Vec<PathBuf>,

    /// Target page size: fit (match image dimensions), a4, or letter
    #[arg(long, default_value = "fit")]
    pub page_size: String,

    /// Page margin in PDF points (0..=200)
    #[arg(long, default_value_t = 0.0)]
    pub margin: f32,

    /// Target output PDF file
    #[arg(short, long)]
    pub out: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct ConvertToMarkdownArgs {
    /// Input PDF file
    pub input: PathBuf,

    #[command(flatten)]
    pub password_opts: OpenPasswordArgs,

    /// Optional 1-based page ranges to extract (omit to extract all pages)
    #[arg(long)]
    pub pages: Option<String>,

    /// Target output Markdown file (defaults to {stem}.md)
    #[arg(short, long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct ConvertFromMarkdownArgs {
    /// Input Markdown (.md) file
    pub input: PathBuf,

    /// Target page size: a4 or letter
    #[arg(long, default_value = "a4")]
    pub page_size: String,

    /// Page margin in PDF points (18..=144)
    #[arg(long, default_value_t = 54.0)]
    pub margin: f32,

    /// Target output PDF file (defaults to {stem}.pdf)
    #[arg(short, long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct WatermarkArgs {
    /// Input PDF file
    pub input: PathBuf,

    #[command(flatten)]
    pub password_opts: OpenPasswordArgs,

    /// Text to stamp as a watermark
    #[arg(long)]
    pub text: String,

    /// Watermark placement: diagonal, center, top-left, top-right, bottom-left, or bottom-right
    #[arg(long, default_value = "diagonal")]
    pub position: String,

    /// Font size in PDF points (6..=300)
    #[arg(long, default_value_t = 48.0)]
    pub font_size: f32,

    /// Opacity between 0.05 and 1.0
    #[arg(long, default_value_t = 0.25)]
    pub opacity: f32,

    /// Optional explicit rotation angle in degrees (overrides position default)
    #[arg(long, allow_hyphen_values = true)]
    pub rotation: Option<f32>,

    /// Color as #RRGGBB hex or preset name (gray, red, blue, black, green)
    #[arg(long, default_value = "#6B7280")]
    pub color: String,

    /// Optional 1-based page ranges to watermark (omit to watermark all pages)
    #[arg(long)]
    pub pages: Option<String>,

    /// Target output PDF file (defaults to {stem}_watermarked.pdf)
    #[arg(short, long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct NumberArgs {
    /// Input PDF file
    pub input: PathBuf,

    #[command(flatten)]
    pub password_opts: OpenPasswordArgs,

    /// Page number format template supporting {n}, {total}, {page}, and {pages}
    #[arg(long, default_value = "{n}")]
    pub format: String,

    /// Page number placement: bottom-center, bottom-right, bottom-left, top-center, top-right, or top-left
    #[arg(long, default_value = "bottom-center")]
    pub position: String,

    /// Starting sequence number for {n} on the first selected page
    #[arg(long, default_value_t = 1)]
    pub start: u32,

    /// Font size in PDF points (4..=72)
    #[arg(long, default_value_t = 10.0)]
    pub font_size: f32,

    /// Opacity between 0.05 and 1.0
    #[arg(long, default_value_t = 1.0)]
    pub opacity: f32,

    /// Color as #RRGGBB hex or preset name (gray, red, blue, black, green)
    #[arg(long, default_value = "#444444")]
    pub color: String,

    /// Margin inset from page edges in PDF points (0..=200)
    #[arg(long, default_value_t = 28.0)]
    pub margin: f32,

    /// Optional 1-based page ranges to number (omit to number all pages)
    #[arg(long)]
    pub pages: Option<String>,

    /// Target output PDF file (defaults to {stem}_numbered.pdf)
    #[arg(short, long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct ProtectArgs {
    /// Input PDF file
    pub input: PathBuf,

    /// Legacy user password flag (removed for security)
    #[arg(long, hide = true)]
    pub password: Option<String>,

    /// Legacy owner password flag (removed for security)
    #[arg(long, hide = true)]
    pub owner_password: Option<String>,

    /// Legacy user password alias (removed for security)
    #[arg(long, hide = true)]
    pub user_password: Option<String>,

    #[command(flatten)]
    pub password_opts: OpenPasswordArgs,

    #[command(flatten)]
    pub new_password: NewPasswordArgs,

    /// Disallow printing the protected PDF (Hinweis an den Viewer)
    #[arg(long)]
    pub no_print: bool,

    /// Disallow copying text and graphics from the protected PDF (Hinweis an den Viewer)
    #[arg(long)]
    pub no_copy: bool,

    /// Disallow modifying contents, annotations, and forms in the protected PDF (Hinweis an den Viewer)
    #[arg(long)]
    pub no_modify: bool,

    /// Target output PDF file (defaults to {stem}_protected.pdf)
    #[arg(short, long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct UnlockArgs {
    /// Encrypted input PDF file
    pub input: PathBuf,

    /// Legacy password flag (removed for security)
    #[arg(long, hide = true)]
    pub password: Option<String>,

    /// Legacy user password alias (removed for security)
    #[arg(long, hide = true)]
    pub user_password: Option<String>,

    #[command(flatten)]
    pub password_opts: OpenPasswordArgs,

    /// Target output PDF file (defaults to {stem}_unlocked.pdf)
    #[arg(short, long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct MetaArgs {
    #[command(subcommand)]
    pub action: MetaCommands,
}

#[derive(Debug, Clone, Subcommand)]
pub enum MetaCommands {
    /// Read standard and custom metadata and inspect XMP status
    #[command(
        after_help = "Examples:\n  sheaf meta get report.pdf\n  sheaf meta get a.pdf b.pdf --xmp --json"
    )]
    Get(MetaGetArgs),

    /// Update document metadata fields, custom keys, and synchronize XMP
    #[command(
        after_help = "Examples:\n  sheaf meta set report.pdf --title \"Q3 Report\" --author \"Müller\" -o updated.pdf\n  sheaf meta set doc.pdf --custom Department=Finance --unset Producer -o clean_prod.pdf --json"
    )]
    Set(Box<MetaSetArgs>),

    /// Strip metadata, PieceInfo, and object-level metadata from PDF
    #[command(
        after_help = "Examples:\n  sheaf meta strip input.pdf -o cleaned.pdf\n  sheaf meta strip sensitive.pdf --deep --keep Title -o anonymized.pdf --json\n\nHinweis:\n  Strip entfernt keine sichtbaren Inhalte, eingebetteten Schriftnamen, Bild-EXIF oder Dateinamen eingebetteter Anhänge."
    )]
    Strip(MetaStripArgs),
}

#[derive(Debug, Clone, Args)]
pub struct MetaGetArgs {
    /// PDF files to inspect metadata from
    #[arg(required = true)]
    pub input: Vec<PathBuf>,

    /// Include full raw XMP packet string (up to 1MB)
    #[arg(long)]
    pub xmp: bool,

    #[command(flatten)]
    pub password_opts: OpenPasswordArgs,
}

#[derive(Debug, Clone, Args)]
pub struct MetaSetArgs {
    /// Input PDF file to modify metadata for
    pub input: PathBuf,

    /// Set document title
    #[arg(long)]
    pub title: Option<String>,

    /// Set document author
    #[arg(long)]
    pub author: Option<String>,

    /// Set document subject
    #[arg(long)]
    pub subject: Option<String>,

    /// Set document keywords
    #[arg(long)]
    pub keywords: Option<String>,

    /// Set document creator tool
    #[arg(long)]
    pub creator: Option<String>,

    /// Set document PDF producer
    #[arg(long)]
    pub producer: Option<String>,

    /// Set creation date (ISO-8601 string)
    #[arg(long)]
    pub creation_date: Option<String>,

    /// Set modification date ('now', 'keep', or ISO-8601 string)
    #[arg(long, default_value = "now")]
    pub mod_date: String,

    /// Set custom key-value pairs (repeatable, format KEY=VALUE)
    #[arg(long = "custom", value_name = "KEY=VALUE")]
    pub custom: Vec<String>,

    /// Remove a metadata key (repeatable, standard or custom)
    #[arg(long = "unset", value_name = "KEY")]
    pub unset: Vec<String>,

    /// XMP synchronization mode (sync, keep, drop)
    #[arg(long, default_value = "sync", value_parser = ["sync", "keep", "drop"])]
    pub xmp: String,

    /// Target output PDF file (defaults to {stem}_meta.pdf)
    #[arg(short, long)]
    pub out: Option<PathBuf>,

    #[command(flatten)]
    pub password_opts: OpenPasswordArgs,
}

#[derive(Debug, Clone, Args)]
pub struct MetaStripArgs {
    /// Input PDF file to strip metadata from
    pub input: PathBuf,

    /// Also strip metadata from annotations (/T, /M, /CreationDate, /Subj)
    #[arg(long)]
    pub deep: bool,

    /// Preserve specific metadata fields in the Info dictionary (repeatable)
    #[arg(long = "keep", value_name = "FIELD")]
    pub keep: Vec<String>,

    /// Target output PDF file (defaults to {stem}_clean.pdf)
    #[arg(short, long)]
    pub out: Option<PathBuf>,

    #[command(flatten)]
    pub password_opts: OpenPasswordArgs,
}

#[derive(Debug, Clone, Args)]
pub struct InfoArgs {
    /// PDF files to inspect
    #[arg(required = true)]
    pub files: Vec<PathBuf>,

    #[command(flatten)]
    pub password_opts: OpenPasswordArgs,
}

#[derive(Debug, Clone, Args)]
pub struct VerifyArgs {
    /// PDF files to verify
    #[arg(required = true)]
    pub files: Vec<PathBuf>,

    #[command(flatten)]
    pub password_opts: OpenPasswordArgs,

    /// Also run qpdf --check if installed
    #[arg(long)]
    pub deep: bool,
}

#[derive(Debug, Clone, Default, Args)]
pub struct CapabilitiesArgs {}

/// Arguments for `sheaf completions`.
#[derive(Debug, Clone, Args)]
pub struct CompletionsArgs {
    /// Target shell to generate completions for (bash, zsh, fish, elvish, powershell)
    #[arg(value_enum)]
    pub shell: ShellChoice,
}

/// Supported shell dialects for tab completion.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum ShellChoice {
    Bash,
    Zsh,
    Fish,
    Elvish,
    Powershell,
}

#[derive(Debug, Clone, Default, Args)]
pub struct DocsArgs {}
