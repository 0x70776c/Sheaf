use sheaf_core::{SheafError, WatermarkOptions, WatermarkPosition, WatermarkReport, watermark};

use crate::cli::WatermarkArgs;
use crate::commands::{Command, Ctx, Outcome};

impl Command for WatermarkArgs {
    type Report = WatermarkReport;

    fn name(&self) -> &'static str {
        "watermark"
    }

    fn run(&self, ctx: &Ctx) -> Result<Outcome<Self::Report>, SheafError> {
        let position = WatermarkPosition::parse(&self.position)?;
        let opts = WatermarkOptions {
            text: self.text.clone(),
            position,
            font_size: self.font_size,
            opacity: self.opacity,
            rotation_deg: self.rotation,
            color: self.color.clone(),
            pages: self.pages.clone(),
            out: self.out.clone(),
            force: ctx.global.force,
            dry_run: ctx.global.dry_run,
        };
        let report = watermark(&self.input, &opts, &ctx.progress)?;

        Ok(Outcome {
            report,
            warnings: Vec::new(),
            dry_run: ctx.global.dry_run,
            failure: None,
        })
    }

    fn format_human(&self, report: &Self::Report, dry_run: bool) -> String {
        let prefix = if dry_run {
            "✓ watermark (dry-run)"
        } else {
            "✓ watermark"
        };
        format!(
            "{prefix}  \"{}\" ({}, {:.0}% opacity, {} pages) → {}",
            report.text,
            report.position,
            report.opacity * 100.0,
            report.pages_watermarked.len(),
            report.output.display()
        )
    }
}
