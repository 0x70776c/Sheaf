use sheaf_core::SheafError;
use sheaf_core::ops::merge::{MergeOptions, MergeReport, merge_with_options};

use crate::cli::MergeArgs;
use crate::commands::{Command, Ctx, Outcome};

impl Command for MergeArgs {
    type Report = MergeReport;

    fn name(&self) -> &'static str {
        "merge"
    }

    fn run(&self, ctx: &Ctx) -> Result<Outcome<Self::Report>, SheafError> {
        let opts = MergeOptions {
            force: ctx.global.force,
            dry_run: ctx.global.dry_run,
        };
        let report = merge_with_options(&self.inputs, &self.out, &opts, &ctx.progress)?;
        Ok(Outcome {
            report,
            warnings: Vec::new(),
            dry_run: ctx.global.dry_run,
            failure: None,
        })
    }

    fn format_human(&self, report: &Self::Report, dry_run: bool) -> String {
        let prefix = if dry_run {
            "✓ merge (dry-run)"
        } else {
            "✓ merge"
        };
        format!(
            "{prefix}  {} files ({} pages) → {}",
            report.inputs.len(),
            report.pages_total,
            report.output.display()
        )
    }
}
