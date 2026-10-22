//! Self-description of Sheaf CLI capabilities (`capabilities`).

use std::collections::BTreeMap;

use serde::Serialize;

use crate::SCHEMA_VERSION;
use crate::ops::verify::find_qpdf;

/// Kind of a CLI argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArgumentKind {
    /// Positional argument (e.g. `<INPUT>`).
    Positional,
    /// Named option that takes a value (e.g. `--out <OUT>`).
    Option,
    /// Boolean switch without a value (e.g. `--force`).
    Flag,
}

/// Single argument capability entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArgumentCapability {
    /// Argument identifier (always set for positional arguments).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Long option name (e.g. `"password"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long: Option<String>,
    /// Value placeholder name (e.g. `"PASSWORD"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_name: Option<String>,
    /// Whether the argument is required.
    pub required: bool,
    /// Whether the argument carries secret data (passwords/keys).
    pub secret: bool,
    /// Argument kind: positional, option or flag.
    pub kind: ArgumentKind,
    /// Default value as configured in the CLI definition.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    /// Allowed values for enum-like arguments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub possible_values: Option<Vec<String>>,
    /// First line of the help text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
}

/// Description of a single CLI subcommand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandCapability {
    /// Full leaf command path separated by spaces (e.g. `"pages rotate"`).
    pub name: String,
    /// First line of the command description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    /// Supported command-specific arguments.
    pub args: Vec<ArgumentCapability>,
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
    /// Global arguments accepted by every command (not repeated per command).
    pub global_args: Vec<ArgumentCapability>,
    /// List of available leaf commands.
    pub commands: Vec<CommandCapability>,
    /// Mapping of numeric exit codes to their primary stable machine code.
    pub exit_codes: BTreeMap<String, &'static str>,
    /// Mapping of every stable JSON error code to its numeric exit code.
    ///
    /// Several error codes may share one exit code (e.g. `encrypted` and `wrong_password` → 4).
    pub error_codes: BTreeMap<&'static str, u8>,
    /// External tool availability.
    pub tools: ToolAvailability,
}

/// Standard exit code mapping for Sheaf.
pub fn default_exit_codes() -> BTreeMap<String, &'static str> {
    BTreeMap::from([
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
    ])
}

/// Mapping of every stable JSON error code to its numeric exit code.
pub fn default_error_codes() -> BTreeMap<&'static str, u8> {
    BTreeMap::from([
        ("internal", 1),
        ("usage", 2),
        ("invalid_selection", 2),
        ("empty_selection", 2),
        ("invalid_pdf", 3),
        ("encrypted", 4),
        ("wrong_password", 4),
        ("page_out_of_range", 5),
        ("io", 6),
        ("output_exists", 7),
        ("unsupported", 8),
        ("cancelled", 130),
    ])
}

/// Builds a [`CapabilitiesReport`] describing the current Sheaf core capabilities, exit codes, and tools.
///
/// # Examples
///
/// ```
/// use sheaf_core::ops::capabilities::capabilities;
///
/// let cap = capabilities();
/// assert_eq!(cap.schema, 1);
/// assert!(cap.commands.iter().any(|c| c.name == "render"));
/// ```
pub fn capabilities() -> CapabilitiesReport {
    let commands = vec![
        "merge",
        "split",
        "pages",
        "render",
        "compress",
        "convert",
        "watermark",
        "number",
        "protect",
        "unlock",
        "info",
        "verify",
        "capabilities",
    ]
    .into_iter()
    .map(|name| CommandCapability {
        name: name.to_string(),
        help: None,
        args: Vec::new(),
        status: "stable",
    })
    .collect();

    CapabilitiesReport {
        version: env!("CARGO_PKG_VERSION"),
        schema: SCHEMA_VERSION,
        global_args: Vec::new(),
        commands,
        exit_codes: default_exit_codes(),
        error_codes: default_error_codes(),
        tools: ToolAvailability {
            qpdf: find_qpdf().is_some(),
        },
    }
}
