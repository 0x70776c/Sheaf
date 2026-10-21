use sheaf_core::{
    ConvertPageSize, ConvertReport, ImageFormat, ImagesToPdfOptions, MarkdownToPdfOptions,
    PdfToImagesOptions, PdfToMarkdownOptions, SheafError, images_to_pdf, markdown_to_pdf,
    pdf_to_images, pdf_to_markdown,
};

use crate::cli::{ConvertArgs, ConvertSubcommand};
use crate::commands::{Command, Ctx, Outcome};

impl Command for ConvertArgs {
    type Report = ConvertReport;

    fn name(&self) -> &'static str {
        "convert"
    }

    fn run(&self, ctx: &Ctx) -> Result<Outcome<Self::Report>, SheafError> {
        let report = match &self.action {
            ConvertSubcommand::ToImages(args) => {
                let mut resolver = crate::password::PasswordResolver::new();
                let pw = resolver.resolve_open_password(&args.password_opts)?;
                let load_opts = sheaf_core::LoadOptions { password: pw };
                let format = ImageFormat::parse(&args.format)?;
                let opts = PdfToImagesOptions {
                    load: load_opts,
                    format,
                    dpi: args.dpi,
                    quality: args.quality,
                    pages: args.pages.clone(),
                    out_dir: args.out.clone(),
                    force: ctx.global.force,
                    dry_run: ctx.global.dry_run,
                };
                ConvertReport::ToImages(pdf_to_images(&args.input, &opts, &ctx.progress)?)
            }
            ConvertSubcommand::FromImages(args) => {
                let page_size = ConvertPageSize::parse(&args.page_size)?;
                let opts = ImagesToPdfOptions {
                    page_size,
                    margin_pt: args.margin,
                    out: args.out.clone(),
                    force: ctx.global.force,
                    dry_run: ctx.global.dry_run,
                };
                ConvertReport::FromImages(images_to_pdf(&args.inputs, &opts, &ctx.progress)?)
            }
            ConvertSubcommand::ToMarkdown(args) => {
                let mut resolver = crate::password::PasswordResolver::new();
                let pw = resolver.resolve_open_password(&args.password_opts)?;
                let load_opts = sheaf_core::LoadOptions { password: pw };
                let opts = PdfToMarkdownOptions {
                    load: load_opts,
                    pages: args.pages.clone(),
                    out: args.out.clone(),
                    force: ctx.global.force,
                    dry_run: ctx.global.dry_run,
                };
                ConvertReport::ToMarkdown(pdf_to_markdown(&args.input, &opts, &ctx.progress)?)
            }
            ConvertSubcommand::FromMarkdown(args) => {
                let page_size = ConvertPageSize::parse(&args.page_size)?;
                let opts = MarkdownToPdfOptions {
                    page_size,
                    margin_pt: args.margin,
                    out: args.out.clone(),
                    force: ctx.global.force,
                    dry_run: ctx.global.dry_run,
                };
                ConvertReport::FromMarkdown(markdown_to_pdf(&args.input, &opts, &ctx.progress)?)
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
        let dry_suffix = if dry_run { " (dry-run)" } else { "" };
        match report {
            ConvertReport::ToImages(r) => format!(
                "✓ convert to-images{dry_suffix}  {} ({} {} files @ {} DPI) → {}",
                r.input.display(),
                r.outputs.len(),
                r.format.to_ascii_uppercase(),
                r.dpi,
                r.output_dir.display()
            ),
            ConvertReport::FromImages(r) => format!(
                "✓ convert from-images{dry_suffix}  {} images ({} pages, {}) → {}",
                r.inputs.len(),
                r.pages,
                r.page_size,
                r.output.display()
            ),
            ConvertReport::ToMarkdown(r) => format!(
                "✓ convert to-markdown{dry_suffix}  {} ({} pages, {} lines) → {}",
                r.input.display(),
                r.extracted_pages,
                r.line_count,
                r.output.display()
            ),
            ConvertReport::FromMarkdown(r) => format!(
                "✓ convert from-markdown{dry_suffix}  {} ({} pages, {} headings) → {}",
                r.input.display(),
                r.pages,
                r.headings_count,
                r.output.display()
            ),
        }
    }
}
