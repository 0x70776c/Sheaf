use std::fmt::Write as _;

use clap::CommandFactory;
use sheaf_core::SCHEMA_VERSION;
use sheaf_core::SheafError;
use sheaf_core::ops::capabilities::{
    ArgumentCapability, CapabilitiesReport, CommandCapability, ToolAvailability, default_exit_codes,
};
use sheaf_core::ops::verify::find_qpdf;

use crate::cli::{CapabilitiesArgs, Cli};
use crate::commands::{Command, Ctx, Outcome};

/// Introspects Clap command metadata to generate the [`CapabilitiesReport`].
pub fn generate_capabilities_report() -> CapabilitiesReport {
    let root = Cli::command();
    let mut commands = Vec::new();

    for sub in root.get_subcommands() {
        if sub.is_hide_set() {
            continue;
        }
        let name = sub.get_name().to_string();
        let mut args = Vec::new();

        for arg in sub.get_arguments() {
            if arg.get_id() == "help" || arg.get_id() == "version" {
                continue;
            }
            let long = arg.get_long().map(str::to_string);
            let value_name = arg
                .get_value_names()
                .and_then(|names| names.first().map(|s| s.to_string()));
            let required = arg.is_required_set();
            let secret = long
                .as_deref()
                .map(|l| l.contains("password"))
                .unwrap_or(false);

            args.push(ArgumentCapability {
                long,
                value_name,
                required,
                secret,
            });
        }

        commands.push(CommandCapability {
            name,
            args,
            status: "stable",
        });
    }

    CapabilitiesReport {
        version: env!("CARGO_PKG_VERSION"),
        schema: SCHEMA_VERSION,
        commands,
        exit_codes: default_exit_codes(),
        tools: ToolAvailability {
            qpdf: find_qpdf().is_some(),
        },
    }
}

impl Command for CapabilitiesArgs {
    type Report = CapabilitiesReport;

    fn name(&self) -> &'static str {
        "capabilities"
    }

    fn run(&self, ctx: &Ctx) -> Result<Outcome<Self::Report>, SheafError> {
        let report = generate_capabilities_report();
        Ok(Outcome {
            report,
            warnings: Vec::new(),
            dry_run: ctx.global.dry_run,
            failure: None,
        })
    }

    fn format_human(&self, report: &Self::Report, _dry_run: bool) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "sheaf v{} (Schema {})", report.version, report.schema);
        let _ = writeln!(out, "Commands:");
        for cmd in &report.commands {
            let arg_summary: Vec<String> = cmd
                .args
                .iter()
                .map(|a| {
                    if let Some(l) = &a.long {
                        format!("--{l}")
                    } else if let Some(v) = &a.value_name {
                        format!("<{v}>")
                    } else {
                        "-".to_string()
                    }
                })
                .collect();
            let _ = writeln!(
                out,
                "  {:<14} [{}] {}",
                cmd.name,
                cmd.status,
                arg_summary.join(" ")
            );
        }
        let _ = write!(out, "Tools: qpdf={}", report.tools.qpdf);
        out
    }
}
