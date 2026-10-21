use sheaf_core::{NumberOptions, NumberPosition, NumberReport, SheafError, number};

use crate::cli::NumberArgs;
use crate::commands::{Command, Ctx, Outcome};

impl Command for NumberArgs {
    type Report = NumberReport;

    fn name(&self) -> &'static str {
        "number"
    }

    fn run(&self, ctx: &Ctx) -> Result<Outcome<Self::Report>, SheafError> {
        let mut resolver = crate::password::PasswordResolver::new();
        let pw = resolver.resolve_open_password(&self.password_opts)?;
        let load_opts = sheaf_core::LoadOptions { password: pw };
        let position = NumberPosition::parse(&self.position)?;
        let opts = NumberOptions {
            load: load_opts,
            format: self.format.clone(),
            position,
            start: self.start,
            font_size: self.font_size,
            opacity: self.opacity,
            color: self.color.clone(),
            margin: self.margin,
            pages: self.pages.clone(),
            out: self.out.clone(),
            force: ctx.global.force,
            dry_run: ctx.global.dry_run,
        };
        let report = number(&self.input, &opts, &ctx.progress)?;

        Ok(Outcome {
            report,
            warnings: resolver.warnings,
            dry_run: ctx.global.dry_run,
            failure: None,
        })
    }

    fn format_human(&self, report: &Self::Report, dry_run: bool) -> String {
        let prefix = if dry_run {
            "✓ number (dry-run)"
        } else {
            "✓ number"
        };
        format!(
            "{prefix}  \"{}\" ({}, start={}, {} pages) → {}",
            report.format,
            report.position,
            report.start,
            report.pages_numbered.len(),
            report.output.display()
        )
    }
}
