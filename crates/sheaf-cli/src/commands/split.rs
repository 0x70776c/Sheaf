use std::fmt::Write as _;
use std::path::Path;

use sheaf_core::{PageSelection, SheafError, SplitMode, SplitOptions, SplitReport, info, split};

use crate::cli::SplitArgs;
use crate::commands::{Command, Ctx, Outcome};

impl Command for SplitArgs {
    type Report = SplitReport;

    fn name(&self) -> &'static str {
        "split"
    }

    fn run(&self, ctx: &Ctx) -> Result<Outcome<Self::Report>, SheafError> {
        if self.combine && self.ranges.is_none() {
            return Err(SheafError::InvalidSelection(
                "'--combine' can only be used together with '--ranges'.".to_string(),
            ));
        }
        let mode = if let Some(ref ranges_str) = self.ranges {
            let info_report = info(std::slice::from_ref(&self.input), &sheaf_core::NoProgress)?;
            let file_info = info_report
                .files
                .first()
                .ok_or(SheafError::EmptySelection)?;
            if file_info.encrypted {
                return Err(SheafError::Encrypted);
            }
            let total_pages = file_info.pages.unwrap_or(0);
            let selection = PageSelection::parse(ranges_str, total_pages)?;
            SplitMode::Ranges {
                selection,
                combine: self.combine,
            }
        } else if let Some(n) = self.every {
            SplitMode::EveryN(n)
        } else if let Some(level) = self.bookmarks_level {
            SplitMode::ByBookmarks { level }
        } else {
            return Err(SheafError::InvalidSelection(
                "Please specify a split mode (--ranges, --every, or --bookmarks-level)."
                    .to_string(),
            ));
        };

        let out_dir = self.out.clone().unwrap_or_default();
        let opts = SplitOptions {
            mode,
            out_dir,
            force: ctx.global.force,
            dry_run: ctx.global.dry_run,
        };

        let mut report = split(&self.input, &opts, &ctx.progress)?;
        let warnings = std::mem::take(&mut report.warnings);

        Ok(Outcome {
            report,
            warnings,
            dry_run: ctx.global.dry_run,
            failure: None,
        })
    }

    fn format_human(&self, report: &Self::Report, dry_run: bool) -> String {
        let prefix = if dry_run {
            "✓ split (dry-run)"
        } else {
            "✓ split"
        };
        let out_dir_display = report
            .outputs
            .first()
            .and_then(|o| o.path.parent())
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .display();

        let mut out = format!(
            "{prefix}  {} ({} pages) → {} files in {out_dir_display}/",
            report.input.display(),
            report.input_pages,
            report.outputs.len(),
        );

        for item in &report.outputs {
            let file_name = item.path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let _ = write!(out, "\n  {file_name:<20} {} p.", item.pages);
        }

        out
    }
}
