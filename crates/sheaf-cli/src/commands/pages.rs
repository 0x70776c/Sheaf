use std::path::Path;

use sheaf_core::{
    LoadOptions, NoProgress, PageOpOptions, PageSelection, PagesReport, SheafError, delete_pages,
    extract_pages, reorder_pages, rotate_pages,
};

use crate::cli::{PagesArgs, PagesSubcommand};
use crate::commands::{Command, Ctx, Outcome};
use crate::password::PasswordResolver;

fn parse_selection_for_file(
    input: &Path,
    spec: &str,
    load_opts: &LoadOptions,
) -> Result<PageSelection, SheafError> {
    let info_report =
        sheaf_core::ops::info::info_with_options(&[input.to_path_buf()], load_opts, &NoProgress)?;
    let file_info = info_report
        .files
        .first()
        .ok_or(SheafError::EmptySelection)?;
    if file_info.encrypted && file_info.pages.is_none() {
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
        let mut resolver = PasswordResolver::new();
        let (report, warnings) = match &self.action {
            PagesSubcommand::Rotate(args) => {
                let pw = resolver.resolve_open_password(&args.password_opts)?;
                let load_opts = LoadOptions { password: pw };
                let selection = match &args.pages {
                    Some(spec) => Some(parse_selection_for_file(&args.input, spec, &load_opts)?),
                    None => None,
                };
                let opts = PageOpOptions {
                    load: load_opts,
                    out: args.out.clone(),
                    force: ctx.global.force,
                    dry_run: ctx.global.dry_run,
                };
                (
                    rotate_pages(
                        &args.input,
                        selection.as_ref(),
                        args.degrees,
                        &opts,
                        &ctx.progress,
                    )?,
                    resolver.warnings,
                )
            }
            PagesSubcommand::Delete(args) => {
                let pw = resolver.resolve_open_password(&args.password_opts)?;
                let load_opts = LoadOptions { password: pw };
                let selection = parse_selection_for_file(&args.input, &args.pages, &load_opts)?;
                let opts = PageOpOptions {
                    load: load_opts,
                    out: args.out.clone(),
                    force: ctx.global.force,
                    dry_run: ctx.global.dry_run,
                };
                (
                    delete_pages(&args.input, &selection, &opts, &ctx.progress)?,
                    resolver.warnings,
                )
            }
            PagesSubcommand::Reorder(args) => {
                let pw = resolver.resolve_open_password(&args.password_opts)?;
                let load_opts = LoadOptions { password: pw };
                let order = parse_selection_for_file(&args.input, &args.order, &load_opts)?;
                let opts = PageOpOptions {
                    load: load_opts,
                    out: args.out.clone(),
                    force: ctx.global.force,
                    dry_run: ctx.global.dry_run,
                };
                (
                    reorder_pages(&args.input, &order, &opts, &ctx.progress)?,
                    resolver.warnings,
                )
            }
            PagesSubcommand::Extract(args) => {
                let pw = resolver.resolve_open_password(&args.password_opts)?;
                let load_opts = LoadOptions { password: pw };
                let selection = parse_selection_for_file(&args.input, &args.pages, &load_opts)?;
                let opts = PageOpOptions {
                    load: load_opts,
                    out: args.out.clone(),
                    force: ctx.global.force,
                    dry_run: ctx.global.dry_run,
                };
                (
                    extract_pages(&args.input, &selection, &opts, &ctx.progress)?,
                    resolver.warnings,
                )
            }
        };

        let mut warnings = warnings;
        if report.was_encrypted {
            warnings
                .push("Eingabe war verschlüsselt, Ausgabe ist nicht verschlüsselt.".to_string());
        }

        Ok(Outcome {
            report,
            warnings,
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
