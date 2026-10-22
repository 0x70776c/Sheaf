use clap::CommandFactory;
use clap_complete::{Shell, generate};
use serde::{Deserialize, Serialize};
use sheaf_core::SheafError;

use crate::cli::{Cli, CompletionsArgs, ShellChoice};
use crate::commands::{Command, Ctx, Outcome};

/// Report returned by `sheaf completions`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionsReport {
    /// Shell dialect generated (e.g. "bash", "zsh").
    pub shell: String,
    /// Generated shell completion script.
    pub script: String,
}

impl Command for CompletionsArgs {
    type Report = CompletionsReport;

    fn name(&self) -> &'static str {
        "completions"
    }

    fn run(&self, _ctx: &Ctx) -> Result<Outcome<Self::Report>, SheafError> {
        let shell = match self.shell {
            ShellChoice::Bash => Shell::Bash,
            ShellChoice::Zsh => Shell::Zsh,
            ShellChoice::Fish => Shell::Fish,
            ShellChoice::Elvish => Shell::Elvish,
            ShellChoice::Powershell => Shell::PowerShell,
        };

        let mut cmd = Cli::command();
        let mut buf = Vec::new();
        generate(shell, &mut cmd, "sheaf", &mut buf);
        let script = String::from_utf8_lossy(&buf).into_owned();

        let shell_name = match self.shell {
            ShellChoice::Bash => "bash",
            ShellChoice::Zsh => "zsh",
            ShellChoice::Fish => "fish",
            ShellChoice::Elvish => "elvish",
            ShellChoice::Powershell => "powershell",
        };

        Ok(Outcome {
            report: CompletionsReport {
                shell: shell_name.to_string(),
                script,
            },
            warnings: Vec::new(),
            dry_run: false,
            failure: None,
        })
    }

    fn format_human(&self, report: &Self::Report, _dry_run: bool) -> String {
        report.script.clone()
    }
}
