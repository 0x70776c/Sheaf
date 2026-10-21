use std::fmt::Write as _;

use sheaf_core::{InfoReport, SheafError, info};

use crate::cli::InfoArgs;
use crate::commands::{Command, Ctx, Outcome};

impl Command for InfoArgs {
    type Report = InfoReport;

    fn name(&self) -> &'static str {
        "info"
    }

    fn run(&self, ctx: &Ctx) -> Result<Outcome<Self::Report>, SheafError> {
        let report = info(&self.files, &ctx.progress)?;
        Ok(Outcome {
            report,
            warnings: Vec::new(),
            dry_run: ctx.global.dry_run,
            failure: None,
        })
    }

    fn format_human(&self, report: &Self::Report, _dry_run: bool) -> String {
        let mut out = String::new();
        for (idx, f) in report.files.iter().enumerate() {
            if idx > 0 {
                out.push('\n');
            }
            let pages_str = match f.pages {
                Some(p) => format!("{p} pages"),
                None => "encrypted".to_string(),
            };
            let _ = write!(
                out,
                "{}  (PDF {}, {}, {} bytes)",
                f.path.display(),
                f.pdf_version,
                pages_str,
                f.size_bytes
            );
            if f.has_outlines {
                let _ = write!(out, " · {} bookmarks", f.outline_count);
            }
        }
        out
    }
}
