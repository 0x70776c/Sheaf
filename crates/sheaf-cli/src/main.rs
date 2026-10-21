use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use sheaf_core::{NoProgress, merge};

#[derive(Debug, Parser)]
#[command(
    name = "sheaf",
    version,
    about = "Schnelles, 100 % offline arbeitendes PDF-Toolkit für macOS"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Mehrere PDF-Dateien in einer neuen PDF-Datei zusammenführen
    Merge {
        /// Eingabe-PDFs in der gewünschten Reihenfolge
        #[arg(required = true)]
        inputs: Vec<PathBuf>,

        /// Ausgabedatei
        #[arg(short, long)]
        out: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::Merge { inputs, out } => {
            if let Err(err) = merge(&inputs, &out, &NoProgress) {
                eprintln!("Fehler: {err}");
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
    }
}
