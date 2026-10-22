use sheaf_core::{ProtectOptions, ProtectReport, SheafError, UnlockOptions, protect, unlock};

use crate::cli::{ProtectArgs, UnlockArgs};
use crate::commands::{Command, Ctx, Outcome};
use crate::password::PasswordResolver;

impl Command for ProtectArgs {
    type Report = ProtectReport;

    fn name(&self) -> &'static str {
        "protect"
    }

    fn run(&self, ctx: &Ctx) -> Result<Outcome<Self::Report>, SheafError> {
        if self.password.is_some() || self.user_password.is_some() {
            return Err(SheafError::InvalidSelection(
                "--password wurde entfernt (Sicherheitsrisiko). Nutze --password-env VAR, --password-file PFAD oder --password-stdin.".to_string(),
            ));
        }
        if self.owner_password.is_some() {
            return Err(SheafError::InvalidSelection(
                "--owner-password wurde entfernt (Sicherheitsrisiko). Nutze --new-owner-password-env VAR oder --new-owner-password-file PFAD.".to_string(),
            ));
        }

        let mut resolver = PasswordResolver::new();
        let load_opts = resolver.resolve_load_options(&self.password_opts)?;
        let user_password = resolver.resolve_new_user_password(&self.new_password)?;
        let owner_password = resolver.resolve_new_owner_password(&self.new_password)?;

        let opts = ProtectOptions {
            load: load_opts,
            user_password,
            owner_password,
            allow_print: !self.no_print,
            allow_copy: !self.no_copy,
            allow_modify: !self.no_modify,
            out: self.out.clone(),
            force: ctx.global.force,
            dry_run: ctx.global.dry_run,
        };
        let report = protect(&self.input, &opts, &ctx.progress)?;

        Ok(Outcome {
            report,
            warnings: resolver.warnings,
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
        if self.password.is_some() || self.user_password.is_some() {
            return Err(SheafError::InvalidSelection(
                "--password wurde entfernt (Sicherheitsrisiko). Nutze --password-env VAR, --password-file PFAD oder --password-stdin.".to_string(),
            ));
        }

        let mut resolver = PasswordResolver::new();
        let password = resolver.resolve_open_password(&self.password_opts)?;

        let opts = UnlockOptions {
            password,
            out: self.out.clone(),
            force: ctx.global.force,
            dry_run: ctx.global.dry_run,
        };
        let report = unlock(&self.input, &opts, &ctx.progress)?;

        let mut warnings = resolver.warnings;
        if !report.was_encrypted {
            warnings.push("Datei ist nicht verschlüsselt".to_string());
        } else {
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
