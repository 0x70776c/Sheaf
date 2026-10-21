use std::path::Path;

use sheaf_core::{
    NoProgress, PageOpOptions, PageSelection, PagesReport, SheafError, delete_pages, extract_pages,
    info, reorder_pages, rotate_pages,
};

use crate::cli::{PagesArgs, PagesSubcommand};
use crate::commands::{Command, Ctx, Outcome};

fn parse_selection_for_file(input: &Path, spec: &str) -> Result<PageSelection, SheafError> {
    let info_report = info(&[input.to_path_buf()], &NoProgress)?;
    let file_info = info_report
        .files
        .first()
        .ok_or(SheafError::EmptySelection)?;
    if file_info.encrypted {
        return Err(SheafError::Encrypted);
    }
    let total_pages = file_info.pages.unwrap_or(0);
    PageSelection::parse(spec, total_pages)
}

impl Command for PagesArgs {
    type Report = PagesReport;

    fn name(&self) -> &'static str {
        "pages"
    }

    fn run(&self, ctx: &Ctx) -> Result<Outcome<Self::Report>, SheafError> {
        let report = match &self.action {
            PagesSubcommand::Rotate(args) => {
                let selection = match &args.pages {
                    Some(spec) => Some(parse_selection_for_file(&args.input, spec)?),
                    None => None,
                };
                let opts = PageOpOptions {
                    out: args.out.clone(),
                    force: ctx.global.force,
                    dry_run: ctx.global.dry_run,
                };
                rotate_pages(
                    &args.input,
                    selection.as_ref(),
                    args.degrees,
                    &opts,
                    &ctx.progress,
                )?
            }
            PagesSubcommand::Delete(args) => {
                let selection = parse_selection_for_file(&args.input, &args.pages)?;
                let opts = PageOpOptions {
                    out: args.out.clone(),
                    force: ctx.global.force,
                    dry_run: ctx.global.dry_run,
                };
                delete_pages(&args.input, &selection, &opts, &ctx.progress)?
            }
            PagesSubcommand::Reorder(args) => {
                let order = parse_selection_for_file(&args.input, &args.order)?;
                let opts = PageOpOptions {
                    out: args.out.clone(),
                    force: ctx.global.force,
                    dry_run: ctx.global.dry_run,
                };
                reorder_pages(&args.input, &order, &opts, &ctx.progress)?
            }
            PagesSubcommand::Extract(args) => {
                let selection = parse_selection_for_file(&args.input, &args.pages)?;
                let opts = PageOpOptions {
                    out: args.out.clone(),
                    force: ctx.global.force,
                    dry_run: ctx.global.dry_run,
                };
                extract_pages(&args.input, &selection, &opts, &ctx.progress)?
            }
        };

        Ok(Outcome {
            report,
            warnings: Vec::new(),
            dry_run: ctx.global.dry_run,
            failure: None,
        })
    }

    fn format_human(&self, report: &Self::Report, dry_run: bool) -> String {
        let prefix = if dry_run {
            format!("✓ pages {} (dry-run)", report.action)
        } else {
            format!("✓ pages {}", report.action)
        };

        match report.action {
            "rotate" => format!(
                "{prefix}  {} ({} pages rotated by {}°) → {}",
                report.input.display(),
                report.pages_affected.len(),
                report.degrees.unwrap_or(0),
                report.output.display()
            ),
            _ => format!(
                "{prefix}  {} ({} → {} pages) → {}",
                report.input.display(),
                report.input_pages,
                report.output_pages,
                report.output.display()
            ),
        }
    }
}
