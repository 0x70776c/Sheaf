use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use sheaf_core::{
    DEFAULT_RENDER_DPI, RenderOptions, RenderReport, RenderScale, SheafError, render_with_options,
};

use crate::cli::RenderArgs;
use crate::commands::{Command, Ctx, Outcome};

impl Command for RenderArgs {
    type Report = RenderReport;

    fn name(&self) -> &'static str {
        "render"
    }

    fn run(&self, ctx: &Ctx) -> Result<Outcome<Self::Report>, SheafError> {
        let pages_spec = match (&self.page, &self.pages) {
            (Some(p), None) => Some(p.to_string()),
            (None, Some(ranges)) => Some(ranges.clone()),
            (None, None) => None,
            (Some(_), Some(_)) => {
                return Err(SheafError::InvalidSelection(
                    "Cannot specify both --page and --pages".to_string(),
                ));
            }
        };

        let scale = match (self.width, self.dpi) {
            (Some(w), None) => RenderScale::WidthPx(w),
            (None, Some(d)) => RenderScale::Dpi(d),
            (None, None) => RenderScale::Dpi(DEFAULT_RENDER_DPI),
            (Some(_), Some(_)) => {
                return Err(SheafError::InvalidSelection(
                    "Cannot specify both --width and --dpi".to_string(),
                ));
            }
        };

        let out = match &self.out {
            Some(p) => p.clone(),
            None => default_output_dir(&self.input),
        };

        let opts = RenderOptions {
            pages: pages_spec,
            scale,
            out,
            force: ctx.global.force,
            dry_run: ctx.global.dry_run,
            use_cache: !self.no_cache,
            cache_dir: None,
        };

        let report = render_with_options(&self.input, &opts, &ctx.progress)?;

        Ok(Outcome {
            report,
            warnings: Vec::new(),
            dry_run: ctx.global.dry_run,
            failure: None,
        })
    }

    fn format_human(&self, report: &Self::Report, dry_run: bool) -> String {
        let mut s = String::new();
        let count = report.outputs.len();
        let noun = if count == 1 { "page" } else { "pages" };

        if dry_run {
            let _ = writeln!(
                s,
                "[dry-run] Would render {count} {noun} from '{}' ({} total pages):",
                report.input.display(),
                report.total_pages
            );
        } else {
            let _ = writeln!(
                s,
                "Rendered {count} {noun} from '{}' ({} total pages):",
                report.input.display(),
                report.total_pages
            );
        }

        for out in &report.outputs {
            let cache_tag = if out.cached { " (cached)" } else { "" };
            let _ = writeln!(
                s,
                "  page {} -> {} ({}x{} px){cache_tag}",
                out.page,
                out.path.display(),
                out.width_px,
                out.height_px
            );
        }

        s.trim_end().to_string()
    }
}

fn default_output_dir(input: &Path) -> PathBuf {
    input
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}
