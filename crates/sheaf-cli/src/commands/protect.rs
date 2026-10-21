use sheaf_core::{ProtectOptions, ProtectReport, SheafError, UnlockOptions, protect, unlock};

use crate::cli::{ProtectArgs, UnlockArgs};
use crate::commands::{Command, Ctx, Outcome};

impl Command for ProtectArgs {
    type Report = ProtectReport;

    fn name(&self) -> &'static str {
        "protect"
    }

    fn run(&self, ctx: &Ctx) -> Result<Outcome<Self::Report>, SheafError> {
        let opts = ProtectOptions {
            user_password: self.password.clone(),
            owner_password: self.owner_password.clone(),
            allow_print: !self.no_print,
            allow_copy: !self.no_copy,
            out: self.out.clone(),
            force: ctx.global.force,
            dry_run: ctx.global.dry_run,
        };
        let report = protect(&self.input, &opts, &ctx.progress)?;

        Ok(Outcome {
            report,
            warnings: Vec::new(),
            dry_run: ctx.global.dry_run,
            failure: None,
        })
    }

    fn format_human(&self, report: &Self::Report, dry_run: bool) -> String {
        let prefix = if dry_run {
            "✓ protect (dry-run)"
        } else {
            "✓ protect"
        };
        format!(
            "{prefix}  {} ({} pages encrypted) → {}",
            report.input.display(),
            report.pages,
            report.output.display()
        )
    }
}

impl Command for UnlockArgs {
    type Report = ProtectReport;

    fn name(&self) -> &'static str {
        "unlock"
    }

    fn run(&self, ctx: &Ctx) -> Result<Outcome<Self::Report>, SheafError> {
        let opts = UnlockOptions {
            password: self.password.clone(),
            out: self.out.clone(),
            force: ctx.global.force,
            dry_run: ctx.global.dry_run,
        };
        let report = unlock(&self.input, &opts, &ctx.progress)?;

        Ok(Outcome {
            report,
            warnings: Vec::new(),
            dry_run: ctx.global.dry_run,
            failure: None,
        })
    }

    fn format_human(&self, report: &Self::Report, dry_run: bool) -> String {
        let prefix = if dry_run {
            "✓ unlock (dry-run)"
        } else {
            "✓ unlock"
        };
        format!(
            "{prefix}  {} ({} pages decrypted) → {}",
            report.input.display(),
            report.pages,
            report.output.display()
        )
    }
}
