//! Self-description of Sheaf CLI capabilities (`capabilities`).

use std::collections::BTreeMap;

use serde::Serialize;

use crate::SCHEMA_VERSION;
use crate::ops::verify::find_qpdf;

/// Single argument capability entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArgumentCapability {
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
}

/// Description of a single CLI subcommand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandCapability {
    /// Subcommand name.
    pub name: String,
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
    /// List of available subcommands.
    pub commands: Vec<CommandCapability>,
    /// Mapping of numeric exit codes to stable machine codes.
    pub exit_codes: BTreeMap<String, &'static str>,
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
        args: Vec::new(),
        status: "stable",
    })
    .collect();

    CapabilitiesReport {
        version: env!("CARGO_PKG_VERSION"),
        schema: SCHEMA_VERSION,
        commands,
        exit_codes: default_exit_codes(),
        tools: ToolAvailability {
            qpdf: find_qpdf().is_some(),
        },
    }
}
