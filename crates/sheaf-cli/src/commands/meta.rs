use sheaf_core::{LoadOptions, MetaGetOptions, MetaGetReport, SheafError, meta_get};
use std::fmt::Write as _;

use crate::cli::{MetaArgs, MetaCommands, MetaGetArgs};
use crate::commands::{Command, Ctx, Outcome};
use crate::password::PasswordResolver;

/// Top-level report covering `meta get`, `meta set`, and `meta strip`.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum MetaReport {
    /// Report returned by `sheaf meta get`.
    Get(MetaGetReport),
}

impl Command for MetaArgs {
    type Report = MetaReport;

    fn name(&self) -> &'static str {
        match &self.action {
            MetaCommands::Get(_) => "meta get",
            MetaCommands::Set(_) => "meta set",
            MetaCommands::Strip(_) => "meta strip",
        }
    }

    fn run(&self, ctx: &Ctx) -> Result<Outcome<Self::Report>, SheafError> {
        match &self.action {
            MetaCommands::Get(args) => run_meta_get(args, ctx),
            MetaCommands::Set(_) => Err(SheafError::Unsupported(
                "meta set will be implemented in 11B.3".to_string(),
            )),
            MetaCommands::Strip(_) => Err(SheafError::Unsupported(
                "meta strip will be implemented in 11B.4".to_string(),
            )),
        }
    }

    fn format_human(&self, report: &Self::Report, _dry_run: bool) -> String {
        match report {
            MetaReport::Get(rep) => format_meta_get_human(rep),
        }
    }
}

fn run_meta_get(args: &MetaGetArgs, ctx: &Ctx) -> Result<Outcome<MetaReport>, SheafError> {
    let mut resolver = PasswordResolver::new();
    let pw = resolver.resolve_open_password(&args.password_opts)?;
    let load_opts = LoadOptions { password: pw };

    let opts = MetaGetOptions {
        load: load_opts,
        include_xmp_packet: args.xmp,
    };

    let report = meta_get(&args.input, &opts, &ctx.progress)?;
    let warnings = resolver.warnings;

    // Check for failures across files
    let mut failure = None;
    for file in &report.files {
        if !file.ok
            && let Some(err_str) = &file.error
        {
            if err_str.contains("encrypted") || err_str.contains("password-protected") {
                failure = Some(SheafError::Encrypted);
            } else if err_str.contains("wrong") || err_str.contains("Wrong password") {
                failure = Some(SheafError::WrongPassword);
            } else if failure.is_none() {
                failure = Some(SheafError::InvalidPdf(err_str.clone()));
            }
        }
    }

    Ok(Outcome {
        report: MetaReport::Get(report),
        warnings,
        dry_run: ctx.global.dry_run,
        failure,
    })
}

fn format_meta_get_human(report: &MetaGetReport) -> String {
    let mut out = String::new();
    for (idx, f) in report.files.iter().enumerate() {
        if idx > 0 {
            out.push_str("\n\n");
        }
        let _ = writeln!(out, "{}", f.path);
        if !f.ok {
            let err = f.error.as_deref().unwrap_or("Failed to read metadata");
            let _ = writeln!(out, "  Error: {err}");
            continue;
        }

        let _ = writeln!(
            out,
            "  Title:         {}",
            f.info.title.as_deref().unwrap_or("-")
        );
        let _ = writeln!(
            out,
            "  Author:        {}",
            f.info.author.as_deref().unwrap_or("-")
        );
        let _ = writeln!(
            out,
            "  Subject:       {}",
            f.info.subject.as_deref().unwrap_or("-")
        );
        let _ = writeln!(
            out,
            "  Keywords:      {}",
            f.info.keywords.as_deref().unwrap_or("-")
        );
        let _ = writeln!(
            out,
            "  Creator:       {}",
            f.info.creator.as_deref().unwrap_or("-")
        );
        let _ = writeln!(
            out,
            "  Producer:      {}",
            f.info.producer.as_deref().unwrap_or("-")
        );

        let fmt_date = |d: &sheaf_core::PdfDateInfo| match (&d.iso, &d.raw) {
            (Some(iso), Some(raw)) => format!("{iso} (raw: {raw})"),
            (None, Some(raw)) => format!("unparsed (raw: {raw})"),
            _ => "-".to_string(),
        };
        let _ = writeln!(out, "  CreationDate:  {}", fmt_date(&f.dates.creation_date));
        let _ = writeln!(out, "  ModDate:       {}", fmt_date(&f.dates.mod_date));

        if !f.custom.is_empty() {
            let _ = writeln!(out, "  Custom:");
            for (k, v) in &f.custom {
                let _ = writeln!(out, "    {k}: {v}");
            }
        }

        let xmp_desc = if f.xmp.present {
            let size = f.xmp.size_bytes.unwrap_or(0);
            let sync = match f.xmp.in_sync {
                Some(true) => ", in sync",
                Some(false) => ", out of sync",
                None => "",
            };
            format!("present ({size} bytes{sync})")
        } else {
            "none".to_string()
        };
        let _ = write!(out, "  XMP:           {xmp_desc}");
    }
    out
}
