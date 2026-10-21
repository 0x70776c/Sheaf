use sheaf_core::{
    CompressOptions, CompressPreset, CompressReport, SheafError, compress_with_options,
};

use crate::cli::CompressArgs;
use crate::commands::{Command, Ctx, Outcome};

impl Command for CompressArgs {
    type Report = CompressReport;

    fn name(&self) -> &'static str {
        "compress"
    }

    fn run(&self, ctx: &Ctx) -> Result<Outcome<Self::Report>, SheafError> {
        let mut resolver = crate::password::PasswordResolver::new();
        let pw = resolver.resolve_open_password(&self.password_opts)?;
        let load_opts = sheaf_core::LoadOptions { password: pw };

        let preset = CompressPreset::parse(&self.preset)?;
        let opts = CompressOptions {
            load: load_opts,
            preset,
            grayscale: self.grayscale,
            out: self.out.clone(),
            force: ctx.global.force,
            dry_run: ctx.global.dry_run,
        };

        let report = compress_with_options(&self.input, &opts, &ctx.progress)?;
        let mut warnings = resolver.warnings;
        if report.was_encrypted {
            warnings
                .push("Eingabe war verschlüsselt, Ausgabe ist nicht verschlüsselt.".to_string());
        }
        if report.already_optimal {
            warnings.push(
                "File is already optimal (compressed output was not smaller than original)."
                    .to_string(),
            );
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
            "✓ compress (dry-run)"
        } else {
            "✓ compress"
        };

        if report.already_optimal {
            format!(
                "{prefix}  {} is already optimal ({} B) → {}",
                report.input.display(),
                report.size_before_bytes,
                report.output.display()
            )
        } else {
            format!(
                "{prefix}  {} ({} B → {} B, -{:.1}%, {} images resampled, {} objects deduplicated) → {}",
                report.input.display(),
                report.size_before_bytes,
                report.size_after_bytes,
                report.saved_percent,
                report.images_resampled,
                report.objects_deduped,
                report.output.display()
            )
        }
    }
}
