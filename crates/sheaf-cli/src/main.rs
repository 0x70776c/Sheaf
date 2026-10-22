mod cli;
mod commands;
mod exit;
mod output;
mod password;
mod progress;

use std::io::Write;
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

static JSON_PANIC_OUTPUT: AtomicBool = AtomicBool::new(false);

fn install_panic_hook() {
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if JSON_PANIC_OUTPUT.load(Ordering::SeqCst) {
            let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = info.payload().downcast_ref::<String>() {
                s.clone()
            } else {
                "Internal panic occurred".to_string()
            };

            let envelope = serde_json::json!({
                "schema": 1,
                "ok": false,
                "op": "panic",
                "error": {
                    "code": "internal",
                    "exit_code": 1,
                    "message": msg,
                    "details": null
                }
            });
            let _ = writeln!(std::io::stdout(), "{envelope}");

            if std::env::var("RUST_BACKTRACE")
                .map(|v| v == "1" || v == "full")
                .unwrap_or(false)
            {
                let backtrace = std::backtrace::Backtrace::capture();
                eprintln!("{backtrace}");
            }
            std::process::exit(1);
        } else {
            prev_hook(info);
        }
    }));
}

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
    install_panic_hook();

    let raw_args: Vec<String> = std::env::args().collect();
    let json_requested = raw_args.iter().any(|a| a == "--json");
    JSON_PANIC_OUTPUT.store(json_requested, Ordering::SeqCst);

    if std::env::var("SHEAF_TEST_PANIC").is_ok() || raw_args.iter().any(|a| a == "--test-panic") {
        panic!("SHEAF_TEST_PANIC triggered");
    }

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
        Commands::Meta(args) => execute_command(args, &cli.global, cancelled),
        Commands::Info(args) => execute_command(args, &cli.global, cancelled),
        Commands::Verify(args) => execute_command(args, &cli.global, cancelled),
        Commands::Capabilities(args) => execute_command(args, &cli.global, cancelled),
        Commands::Docs(args) => execute_command(args, &cli.global, cancelled),
        Commands::Completions(args) => execute_command(args, &cli.global, cancelled),
    };

    ExitCode::from(code)
}
