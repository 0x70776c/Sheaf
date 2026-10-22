use std::fmt::Write as _;

use clap::ArgAction;
use clap::CommandFactory;
use sheaf_core::SCHEMA_VERSION;
use sheaf_core::SheafError;
use sheaf_core::ops::capabilities::{
    ArgumentCapability, ArgumentKind, CapabilitiesReport, CommandCapability, ToolAvailability,
    default_error_codes, default_exit_codes,
};
use sheaf_core::ops::verify::find_qpdf;

use crate::cli::{CapabilitiesArgs, Cli};
use crate::commands::{Command, Ctx, Outcome};

fn first_line(text: Option<&clap::builder::StyledStr>) -> Option<String> {
    text.map(|t| t.to_string())
        .and_then(|t| t.lines().next().map(|l| l.trim().to_string()))
        .filter(|l| !l.is_empty())
}

fn is_skipped(arg: &clap::Arg) -> bool {
    arg.get_id() == "help" || arg.get_id() == "version" || arg.is_hide_set()
}

/// Converts one clap argument into an [`ArgumentCapability`].
fn describe_arg(arg: &clap::Arg) -> ArgumentCapability {
    let long = arg.get_long().map(str::to_string);
    let kind = if arg.is_positional() {
        ArgumentKind::Positional
    } else if matches!(
        arg.get_action(),
        ArgAction::SetTrue | ArgAction::SetFalse | ArgAction::Count
    ) {
        ArgumentKind::Flag
    } else {
        ArgumentKind::Option
    };
    let value_name = if kind == ArgumentKind::Flag {
        None
    } else {
        arg.get_value_names()
            .and_then(|names| names.first().map(|s| s.to_string()))
            .or_else(|| Some(arg.get_id().as_str().to_uppercase()))
    };
    let default = if kind == ArgumentKind::Flag {
        None
    } else {
        let vals: Vec<String> = arg
            .get_default_values()
            .iter()
            .map(|v| v.to_string_lossy().into_owned())
            .collect();
        if vals.is_empty() {
            None
        } else {
            Some(vals.join(","))
        }
    };
    let possible: Vec<String> = if kind == ArgumentKind::Flag {
        Vec::new()
    } else {
        arg.get_possible_values()
            .iter()
            .filter(|v| !v.is_hide_set())
            .map(|v| v.get_name().to_string())
            .collect()
    };
    let secret = long
        .as_deref()
        .map(|l| l.contains("password"))
        .unwrap_or(false);

    ArgumentCapability {
        name: if kind == ArgumentKind::Positional {
            Some(arg.get_id().as_str().to_string())
        } else {
            None
        },
        long,
        value_name,
        required: arg.is_required_set(),
        secret,
        kind,
        default,
        possible_values: if possible.is_empty() {
            None
        } else {
            Some(possible)
        },
        help: first_line(arg.get_help()),
    }
}

/// Recursively collects all visible leaf commands below `cmd`.
fn collect_leaves(cmd: &clap::Command, prefix: &str, out: &mut Vec<CommandCapability>) {
    for sub in cmd.get_subcommands() {
        if sub.is_hide_set() {
            continue;
        }
        let name = if prefix.is_empty() {
            sub.get_name().to_string()
        } else {
            format!("{prefix} {}", sub.get_name())
        };
        if sub.has_subcommands() {
            collect_leaves(sub, &name, out);
            continue;
        }
        let args = sub
            .get_arguments()
            .filter(|a| !is_skipped(a) && !a.is_global_set())
            .map(describe_arg)
            .collect();
        out.push(CommandCapability {
            name,
            help: first_line(sub.get_about()),
            args,
            status: "stable",
        });
    }
}

/// Introspects Clap command metadata to generate the [`CapabilitiesReport`].
///
/// Every visible leaf command is listed with its full path (e.g. `"pages rotate"`).
/// Global arguments are listed once in `global_args`.
pub fn generate_capabilities_report() -> CapabilitiesReport {
    let root = Cli::command();
    let global_args = root
        .get_arguments()
        .filter(|a| !is_skipped(a) && a.is_global_set())
        .map(describe_arg)
        .collect();
    let mut commands = Vec::new();
    collect_leaves(&root, "", &mut commands);

    CapabilitiesReport {
        version: env!("CARGO_PKG_VERSION"),
        schema: SCHEMA_VERSION,
        global_args,
        commands,
        exit_codes: default_exit_codes(),
        error_codes: default_error_codes(),
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
                "  {:<22} [{}] {}",
                cmd.name,
                cmd.status,
                arg_summary.join(" ")
            );
        }
        let _ = write!(out, "Tools: qpdf={}", report.tools.qpdf);
        out
    }
}
