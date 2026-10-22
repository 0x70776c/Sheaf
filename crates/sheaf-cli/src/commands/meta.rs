use sheaf_core::{
    LoadOptions, MetaGetOptions, MetaGetReport, MetaSetOptions, MetaSetReport, MetaStripOptions,
    MetaStripReport, SheafError, XmpMode, meta_get, meta_set, meta_strip,
};
use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::cli::{MetaArgs, MetaCommands, MetaGetArgs, MetaSetArgs, MetaStripArgs};
use crate::commands::{Command, Ctx, Outcome};
use crate::password::PasswordResolver;

/// Top-level report covering `meta get`, `meta set`, and `meta strip`.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum MetaReport {
    /// Report returned by `sheaf meta get`.
    Get(MetaGetReport),
    /// Report returned by `sheaf meta set`.
    Set(MetaSetReport),
    /// Report returned by `sheaf meta strip`.
    Strip(MetaStripReport),
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
            MetaCommands::Set(args) => run_meta_set(args, ctx),
            MetaCommands::Strip(args) => run_meta_strip(args, ctx),
        }
    }

    fn format_human(&self, report: &Self::Report, _dry_run: bool) -> String {
        match report {
            MetaReport::Get(rep) => format_meta_get_human(rep),
            MetaReport::Set(rep) => format_meta_set_human(rep),
            MetaReport::Strip(rep) => format_meta_strip_human(rep),
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

fn run_meta_set(args: &MetaSetArgs, ctx: &Ctx) -> Result<Outcome<MetaReport>, SheafError> {
    let has_changes = args.title.is_some()
        || args.author.is_some()
        || args.subject.is_some()
        || args.keywords.is_some()
        || args.creator.is_some()
        || args.producer.is_some()
        || args.creation_date.is_some()
        || !args.custom.is_empty()
        || !args.unset.is_empty()
        || args.mod_date != "now"
        || args.xmp == "drop";

    if !has_changes {
        return Err(SheafError::InvalidSelection(
            "No metadata changes specified. Provide at least one field to set, unset, or update."
                .to_string(),
        ));
    }

    let mut resolver = PasswordResolver::new();
    let pw = resolver.resolve_open_password(&args.password_opts)?;
    let load_opts = LoadOptions { password: pw };

    let mut custom = BTreeMap::new();
    for entry in &args.custom {
        let (k, v) = entry.split_once('=').ok_or_else(|| {
            SheafError::InvalidSelection(format!(
                "Invalid custom metadata format: '{entry}'. Expected KEY=VALUE."
            ))
        })?;
        custom.insert(k.trim().to_string(), v.trim().to_string());
    }

    let xmp_mode = XmpMode::parse(&args.xmp)?;

    let opts = MetaSetOptions {
        load: load_opts,
        title: args.title.clone(),
        author: args.author.clone(),
        subject: args.subject.clone(),
        keywords: args.keywords.clone(),
        creator: args.creator.clone(),
        producer: args.producer.clone(),
        creation_date: args.creation_date.clone(),
        mod_date: args.mod_date.clone(),
        custom,
        unset: args.unset.clone(),
        xmp: xmp_mode,
        out: args.out.clone(),
        force: ctx.global.force,
        dry_run: ctx.global.dry_run,
    };

    let report = meta_set(&args.input, &opts, &ctx.progress)?;
    let mut warnings = resolver.warnings;

    if xmp_mode == XmpMode::Keep && report.xmp_in_sync == Some(false) {
        warnings.push("XMP-Metadaten weichen nach 'keep' von Info-Metadaten ab.".to_string());
    }
    if xmp_mode == XmpMode::Sync && report.xmp_in_sync == Some(false) {
        warnings.push("Bestehendes XMP-Paket konnte nicht synchronisiert werden.".to_string());
    }

    Ok(Outcome {
        report: MetaReport::Set(report),
        warnings,
        dry_run: ctx.global.dry_run,
        failure: None,
    })
}

fn format_meta_set_human(report: &MetaSetReport) -> String {
    let mut out = String::new();
    let action_str = if report.dry_run {
        "Plan for"
    } else {
        "Updated metadata for"
    };
    let _ = writeln!(
        out,
        "{action_str} {} → {} (xmp: {})",
        report.input.display(),
        report.output.display(),
        report.xmp_mode
    );

    if report.diff.is_empty() {
        let _ = write!(out, "  (no changes)");
    } else {
        for (i, entry) in report.diff.iter().enumerate() {
            if i > 0 {
                let _ = writeln!(out);
            }
            let old_str = entry.old.as_deref().unwrap_or("null");
            let new_str = entry.new.as_deref().unwrap_or("null");
            let _ = write!(out, "  {:<14} {} → {}", entry.field, old_str, new_str);
        }
    }
    out
}

fn run_meta_strip(args: &MetaStripArgs, ctx: &Ctx) -> Result<Outcome<MetaReport>, SheafError> {
    let mut resolver = PasswordResolver::new();
    let pw = resolver.resolve_open_password(&args.password_opts)?;
    let load_opts = LoadOptions { password: pw };

    let opts = MetaStripOptions {
        load: load_opts,
        deep: args.deep,
        keep: args.keep.clone(),
        out: args.out.clone(),
        force: ctx.global.force,
        dry_run: ctx.global.dry_run,
    };

    let report = meta_strip(&args.input, &opts, &ctx.progress)?;
    let mut warnings = resolver.warnings;
    warnings.extend(report.warnings.clone());

    Ok(Outcome {
        report: MetaReport::Strip(report),
        warnings,
        dry_run: ctx.global.dry_run,
        failure: None,
    })
}

fn format_meta_strip_human(report: &MetaStripReport) -> String {
    let mut out = String::new();
    let action_str = if report.dry_run {
        "Plan for stripping metadata from"
    } else {
        "Stripped metadata from"
    };
    let _ = writeln!(
        out,
        "{action_str} {} → {}",
        report.input.display(),
        report.output.display()
    );

    let _ = writeln!(
        out,
        "  Info keys removed:       {:?}",
        report.removed.info_keys
    );
    let xmp_status = if report.removed.xmp {
        "removed"
    } else {
        "none"
    };
    let _ = writeln!(out, "  XMP Catalog /Metadata:   {xmp_status}");
    let _ = writeln!(
        out,
        "  PieceInfo dictionaries:  {}",
        report.removed.piece_info
    );
    let _ = writeln!(
        out,
        "  Object-level /Metadata:  {}",
        report.removed.object_metadata
    );
    let _ = writeln!(
        out,
        "  Annotation fields:       {}",
        report.removed.annotation_fields
    );
    let id_status = if report.removed.trailer_id_regenerated {
        "regenerated"
    } else {
        "unchanged"
    };
    let _ = write!(out, "  Trailer /ID:             {id_status}");

    out
}
