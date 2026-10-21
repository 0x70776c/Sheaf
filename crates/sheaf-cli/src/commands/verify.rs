use std::fmt::Write as _;

use sheaf_core::SheafError;
use sheaf_core::ops::verify::{VerifyOptions, VerifyReport, verify};

use crate::cli::VerifyArgs;
use crate::commands::{Command, Ctx, Outcome};
use crate::password::PasswordResolver;

impl Command for VerifyArgs {
    type Report = VerifyReport;

    fn name(&self) -> &'static str {
        "verify"
    }

    fn run(&self, ctx: &Ctx) -> Result<Outcome<Self::Report>, SheafError> {
        let load_opts = PasswordResolver::new().resolve_load_options(&self.password_opts)?;
        let opts = VerifyOptions {
            load: load_opts,
            deep: self.deep,
        };
        let mut report = verify(&self.files, &opts, &ctx.progress)?;
        let warnings = std::mem::take(&mut report.warnings);

        let failure = if report.all_valid() {
            None
        } else if report.files.iter().any(|f| {
            f.problems
                .iter()
                .any(|p| p.starts_with("encrypted:") || p.contains("encrypted (password required)"))
        }) {
            Some(SheafError::Encrypted)
        } else if report.files.iter().any(|f| {
            f.problems
                .iter()
                .any(|p| p.starts_with("wrong_password:") || p.contains("Incorrect password"))
        }) {
            Some(SheafError::WrongPassword)
        } else {
            Some(SheafError::InvalidPdf(
                "At least one PDF file failed verification.".to_string(),
            ))
        };

        Ok(Outcome {
            report,
            warnings,
            dry_run: ctx.global.dry_run,
            failure,
        })
    }

    fn format_human(&self, report: &Self::Report, _dry_run: bool) -> String {
        let mut out = String::new();
        for (idx, f) in report.files.iter().enumerate() {
            if idx > 0 {
                out.push('\n');
            }
            if f.valid {
                let pages = f.pages.unwrap_or(0);
                let _ = write!(out, "✓ {} ({pages} pages)", f.path.display());
            } else {
                let _ = write!(out, "✗ {} ({})", f.path.display(), f.problems.join("; "));
            }
        }
        out
    }
}
