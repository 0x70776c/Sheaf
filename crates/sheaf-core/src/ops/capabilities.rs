//! Self-description of Sheaf CLI capabilities (`capabilities`).

use std::collections::BTreeMap;

use serde::Serialize;

use crate::SCHEMA_VERSION;
use crate::ops::verify::find_qpdf;

/// Description of a single CLI subcommand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandCapability {
    /// Subcommand name.
    pub name: &'static str,
    /// Supported command-specific arguments and flags.
    pub args: Vec<&'static str>,
    /// Stability status (`"stable"`).
    pub status: &'static str,
}

/// Availability of external helper tools on the current machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolAvailability {
    /// Whether `qpdf` is available in `PATH` or standard Homebrew locations.
    pub qpdf: bool,
}

/// Report returned by [`capabilities`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilitiesReport {
    /// Package version of Sheaf.
    pub version: &'static str,
    /// JSON envelope schema version.
    pub schema: u32,
    /// List of available subcommands.
    pub commands: Vec<CommandCapability>,
    /// Mapping of numeric exit codes to stable machine codes.
    pub exit_codes: BTreeMap<String, &'static str>,
    /// External tool availability.
    pub tools: ToolAvailability,
}

/// Builds a [`CapabilitiesReport`] describing the current Sheaf CLI commands, exit codes, and tools.
///
/// # Examples
///
/// ```
/// use sheaf_core::ops::capabilities::capabilities;
///
/// let cap = capabilities();
/// assert_eq!(cap.schema, 1);
/// assert!(cap.commands.iter().any(|c| c.name == "merge"));
/// ```
pub fn capabilities() -> CapabilitiesReport {
    let commands = vec![
        CommandCapability {
            name: "merge",
            args: vec!["inputs...", "-o"],
            status: "stable",
        },
        CommandCapability {
            name: "split",
            args: vec![
                "input",
                "--ranges",
                "--every",
                "--bookmarks-level",
                "--combine",
                "-o",
            ],
            status: "stable",
        },
        CommandCapability {
            name: "pages",
            args: vec![
                "rotate|delete|reorder|extract",
                "input",
                "--pages",
                "--order",
                "--degrees",
                "-o",
            ],
            status: "stable",
        },
        CommandCapability {
            name: "render",
            args: vec![
                "input",
                "--page",
                "--pages",
                "--dpi",
                "--width",
                "--no-cache",
                "-o",
            ],
            status: "stable",
        },
        CommandCapability {
            name: "compress",
            args: vec!["input", "--preset", "--grayscale", "-o"],
            status: "stable",
        },
        CommandCapability {
            name: "convert",
            args: vec![
                "to-images|from-images|to-markdown|from-markdown",
                "input",
                "--format",
                "--dpi",
                "--quality",
                "--pages",
                "--page-size",
                "--margin",
                "-o",
            ],
            status: "stable",
        },
        CommandCapability {
            name: "watermark",
            args: vec![
                "input",
                "--text",
                "--position",
                "--font-size",
                "--opacity",
                "--rotation",
                "--color",
                "--pages",
                "-o",
            ],
            status: "stable",
        },
        CommandCapability {
            name: "protect",
            args: vec![
                "input",
                "--password",
                "--owner-password",
                "--no-print",
                "--no-copy",
                "-o",
            ],
            status: "stable",
        },
        CommandCapability {
            name: "unlock",
            args: vec!["input", "--password", "-o"],
            status: "stable",
        },
        CommandCapability {
            name: "info",
            args: vec!["file..."],
            status: "stable",
        },
        CommandCapability {
            name: "verify",
            args: vec!["file...", "--deep"],
            status: "stable",
        },
        CommandCapability {
            name: "capabilities",
            args: vec![],
            status: "stable",
        },
    ];

    let exit_codes = BTreeMap::from([
        ("0".to_string(), "ok"),
        ("1".to_string(), "internal"),
        ("2".to_string(), "usage"),
        ("3".to_string(), "invalid_pdf"),
        ("4".to_string(), "encrypted"),
        ("5".to_string(), "page_out_of_range"),
        ("6".to_string(), "io"),
        ("7".to_string(), "output_exists"),
        ("8".to_string(), "unsupported"),
        ("130".to_string(), "cancelled"),
    ]);

    CapabilitiesReport {
        version: env!("CARGO_PKG_VERSION"),
        schema: SCHEMA_VERSION,
        commands,
        exit_codes,
        tools: ToolAvailability {
            qpdf: find_qpdf().is_some(),
        },
    }
}
