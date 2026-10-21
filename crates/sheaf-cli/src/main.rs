mod cli;
mod commands;
mod exit;
mod output;
mod progress;

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use clap::Parser;

use crate::cli::{Cli, Commands, GlobalArgs};
use crate::commands::{Command, Ctx};
use crate::exit::EXIT_OK;
use crate::output::{render_clap_error, render_error, render_error_with_data, render_success};
use crate::progress::CliProgress;

fn execute_command<C: Command>(cmd: &C, global: &GlobalArgs, cancelled: Arc<AtomicBool>) -> u8 {
    let op_name = cmd.name();
    let progress = CliProgress::new(op_name, global.progress, global.quiet, cancelled);
    let ctx = Ctx {
        global: global.clone(),
        progress,
    };

    if global.verbose && !global.quiet {
        eprintln!(
            "[debug] Running command '{op_name}' (dry_run={})",
            global.dry_run
        );
    }

    let start = Instant::now();
    match cmd.run(&ctx) {
        Ok(outcome) => {
            let duration_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
            let human = cmd.format_human(&outcome.report, outcome.dry_run);

            if let Some(err) = outcome.failure {
                render_error_with_data(
                    op_name,
                    &outcome.report,
                    &human,
                    outcome.warnings,
                    outcome.dry_run,
                    duration_ms,
                    &err,
                    global,
                    &ctx.progress,
                )
            } else {
                render_success(
                    op_name,
                    &outcome.report,
                    &human,
                    outcome.warnings,
                    outcome.dry_run,
                    duration_ms,
                    global,
                    &ctx.progress,
                );
                EXIT_OK
            }
        }
        Err(err) => render_error(op_name, &err, global),
    }
}

fn main() -> ExitCode {
    let raw_args: Vec<String> = std::env::args().collect();

    let cli = match Cli::try_parse_from(&raw_args) {
        Ok(parsed) => parsed,
        Err(err) => {
            let code = render_clap_error(err, &raw_args);
            return ExitCode::from(code);
        }
    };

    let cancelled = Arc::new(AtomicBool::new(false));
    let cancel_flag = Arc::clone(&cancelled);
    let _ = ctrlc::set_handler(move || {
        cancel_flag.store(true, Ordering::SeqCst);
    });

    let code = match &cli.command {
        Commands::Merge(args) => execute_command(args, &cli.global, cancelled),
        Commands::Split(args) => execute_command(args, &cli.global, cancelled),
        Commands::Pages(args) => execute_command(args, &cli.global, cancelled),
        Commands::Render(args) => execute_command(args, &cli.global, cancelled),
        Commands::Compress(args) => execute_command(args, &cli.global, cancelled),
        Commands::Convert(args) => execute_command(args, &cli.global, cancelled),
        Commands::Watermark(args) => execute_command(args, &cli.global, cancelled),
        Commands::Number(args) => execute_command(args, &cli.global, cancelled),
        Commands::Protect(args) => execute_command(args, &cli.global, cancelled),
        Commands::Unlock(args) => execute_command(args, &cli.global, cancelled),
        Commands::Info(args) => execute_command(args, &cli.global, cancelled),
        Commands::Verify(args) => execute_command(args, &cli.global, cancelled),
        Commands::Capabilities(args) => execute_command(args, &cli.global, cancelled),
        Commands::Docs(args) => execute_command(args, &cli.global, cancelled),
    };

    ExitCode::from(code)
}
