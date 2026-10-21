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
    /// Ausgabe als JSON-Envelope auf stdout
    #[arg(long, global = true)]
    pub json: bool,

    /// Eingaben prüfen und Plan berechnen, aber keine Dateien schreiben
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Bestehende Ausgabedateien überschreiben
    #[arg(long, global = true)]
    pub force: bool,

    /// Keine Human-Ausgabe außer Fehlern
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Erweiterte Debug-Ausgaben auf stderr
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Fortschrittsausgabe auf stderr (Standard: human bei TTY, sonst none)
    #[arg(long, value_enum, global = true)]
    pub progress: Option<ProgressMode>,

    /// Farbige Konsolenausgabe deaktivieren (respektiert auch NO_COLOR)
    #[arg(long, global = true)]
    pub no_color: bool,
}

#[derive(Debug, Parser)]
#[command(
    name = "sheaf",
    version,
    about = "Schnelles, 100 % offline arbeitendes PDF-Toolkit für macOS",
    after_help = "Beispiele:\n  sheaf merge a.pdf b.pdf -o combined.pdf\n  sheaf split report.pdf --ranges \"1-3,8-\" -o out/\n  sheaf info report.pdf --json"
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Mehrere PDF-Dateien in einer neuen PDF-Datei zusammenführen
    #[command(
        after_help = "Beispiele:\n  sheaf merge a.pdf b.pdf -o out.pdf\n  sheaf merge teil1.pdf teil2.pdf -o gesamt.pdf --force --json"
    )]
    Merge(MergeArgs),

    /// Eine PDF-Datei nach Seitenbereichen, Blöcken oder Lesezeichen aufteilen
    #[command(
        after_help = "Beispiele:\n  sheaf split report.pdf --ranges \"1-3,5,8-\" -o out/\n  sheaf split report.pdf --every 2 -o out/\n  sheaf split report.pdf --bookmarks-level 1 -o out/ --json"
    )]
    Split(SplitArgs),

    /// Struktur- und Metadaten-Informationen zu PDF-Dateien anzeigen
    #[command(
        after_help = "Beispiele:\n  sheaf info dokument.pdf\n  sheaf info a.pdf b.pdf --json"
    )]
    Info(InfoArgs),

    /// PDF-Dateien auf strukturelle Gültigkeit prüfen
    #[command(
        after_help = "Beispiele:\n  sheaf verify dokument.pdf\n  sheaf verify *.pdf --deep --json"
    )]
    Verify(VerifyArgs),

    /// Unterstützte Befehle, Flags, Exit-Codes und externe Tools beschreiben
    #[command(after_help = "Beispiele:\n  sheaf capabilities --json\n  sheaf capabilities")]
    Capabilities(CapabilitiesArgs),
}

#[derive(Debug, Clone, Args)]
pub struct MergeArgs {
    /// Eingabe-PDFs in der gewünschten Reihenfolge
    #[arg(required = true)]
    pub inputs: Vec<PathBuf>,

    /// Ausgabedatei
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
    /// Eingabe-PDF-Datei
    pub input: PathBuf,

    /// Seitenbereiche (z. B. "1-3,5,8-")
    #[arg(long)]
    pub ranges: Option<String>,

    /// Alle N Seiten aufteilen
    #[arg(long)]
    pub every: Option<u32>,

    /// Nach Lesezeichen auf Ebene N aufteilen (1 = oberste Ebene)
    #[arg(long)]
    pub bookmarks_level: Option<u8>,

    /// Ausgewählte Seitenbereiche in einer einzigen Datei zusammenfassen (nur mit --ranges)
    #[arg(long, requires = "ranges", conflicts_with_all = ["every", "bookmarks_level"])]
    pub combine: bool,

    /// Zielverzeichnis für die erzeugten PDF-Dateien (Standard: Ordner der Eingabedatei)
    #[arg(short, long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct InfoArgs {
    /// Zu untersuchende PDF-Dateien
    #[arg(required = true)]
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct VerifyArgs {
    /// Zu prüfende PDF-Dateien
    #[arg(required = true)]
    pub files: Vec<PathBuf>,

    /// Zusätzlich qpdf --check ausführen (falls installiert)
    #[arg(long)]
    pub deep: bool,
}

#[derive(Debug, Clone, Default, Args)]
pub struct CapabilitiesArgs {}
