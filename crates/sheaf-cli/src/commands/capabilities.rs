use std::fmt::Write as _;

use sheaf_core::SheafError;
use sheaf_core::ops::capabilities::{CapabilitiesReport, capabilities};

use crate::cli::CapabilitiesArgs;
use crate::commands::{Command, Ctx, Outcome};

impl Command for CapabilitiesArgs {
    type Report = CapabilitiesReport;

    fn name(&self) -> &'static str {
        "capabilities"
    }

    fn run(&self, ctx: &Ctx) -> Result<Outcome<Self::Report>, SheafError> {
        let report = capabilities();
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
            let _ = writeln!(
                out,
                "  {:<14} [{}] {}",
                cmd.name,
                cmd.status,
                cmd.args.join(" ")
            );
        }
        let _ = write!(out, "Tools: qpdf={}", report.tools.qpdf);
        out
    }
}
