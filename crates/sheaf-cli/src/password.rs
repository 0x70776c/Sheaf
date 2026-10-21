use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use clap::Args;
use secrecy::SecretString;
use sheaf_core::SheafError;

/// CLI arguments for providing a password to open/decrypt an input PDF.
#[derive(Debug, Clone, Args, Default)]
pub struct OpenPasswordArgs {
    /// Read the open password from an environment variable
    #[arg(long, group = "open_password_group")]
    pub password_env: Option<String>,

    /// Read the open password from a file
    #[arg(long, group = "open_password_group")]
    pub password_file: Option<PathBuf>,

    /// Read the open password from standard input
    #[arg(long, group = "open_password_group")]
    pub password_stdin: bool,
}

/// CLI arguments for providing user and owner passwords when protecting a PDF.
#[derive(Debug, Clone, Args, Default)]
pub struct NewPasswordArgs {
    /// Read the new user password from an environment variable
    #[arg(long, group = "new_user_password_group")]
    pub new_password_env: Option<String>,

    /// Read the new user password from a file
    #[arg(long, group = "new_user_password_group")]
    pub new_password_file: Option<PathBuf>,

    /// Read the new user password from standard input
    #[arg(long, group = "new_user_password_group")]
    pub new_password_stdin: bool,

    /// Read the optional new owner password from an environment variable
    #[arg(long, group = "new_owner_password_group")]
    pub new_owner_password_env: Option<String>,

    /// Read the optional new owner password from a file
    #[arg(long, group = "new_owner_password_group")]
    pub new_owner_password_file: Option<PathBuf>,
}

/// Resolves passwords from environment variables, files, or stdin with strict security rules.
#[derive(Debug, Default)]
pub struct PasswordResolver {
    stdin_read: bool,
    pub warnings: Vec<String>,
}

impl PasswordResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolves an open password for reading/decrypting input PDFs.
    pub fn resolve_open_password(
        &mut self,
        args: &OpenPasswordArgs,
    ) -> Result<Option<SecretString>, SheafError> {
        self.resolve_secret(
            args.password_env.as_deref(),
            args.password_file.as_deref(),
            args.password_stdin,
            "open password",
        )
    }

    /// Resolves the required new user password for `protect`.
    pub fn resolve_new_user_password(
        &mut self,
        args: &NewPasswordArgs,
    ) -> Result<SecretString, SheafError> {
        let opt = self.resolve_secret(
            args.new_password_env.as_deref(),
            args.new_password_file.as_deref(),
            args.new_password_stdin,
            "new user password",
        )?;
        opt.ok_or_else(|| {
            SheafError::InvalidSelection(
                "Missing required password. Specify --new-password-env VAR, --new-password-file PATH, or --new-password-stdin.".to_string(),
            )
        })
    }

    /// Resolves the optional new owner password for `protect`.
    pub fn resolve_new_owner_password(
        &mut self,
        args: &NewPasswordArgs,
    ) -> Result<Option<SecretString>, SheafError> {
        self.resolve_secret(
            args.new_owner_password_env.as_deref(),
            args.new_owner_password_file.as_deref(),
            false,
            "new owner password",
        )
    }

    fn resolve_secret(
        &mut self,
        env_var: Option<&str>,
        file_path: Option<&Path>,
        from_stdin: bool,
        desc: &str,
    ) -> Result<Option<SecretString>, SheafError> {
        if from_stdin {
            if self.stdin_read {
                return Err(SheafError::InvalidSelection(
                    "Only one password may be read from stdin per invocation.".to_string(),
                ));
            }
            if std::io::stdin().is_terminal() {
                return Err(SheafError::InvalidSelection(
                    "Cannot read password from stdin: stdin is an interactive terminal. Hint: pipe the password into sheaf, e.g.: printf '%s' \"$PW\" | sheaf ...".to_string(),
                ));
            }
            let mut line = String::new();
            let n = std::io::stdin().read_line(&mut line)?;
            if n == 0 {
                return Err(SheafError::InvalidSelection(format!(
                    "Password for {desc} from stdin is empty (EOF reached)."
                )));
            }
            if line.ends_with("\r\n") {
                line.truncate(line.len() - 2);
            } else if line.ends_with('\n') {
                line.truncate(line.len() - 1);
            }
            if line.is_empty() {
                return Err(SheafError::InvalidSelection(format!(
                    "Password for {desc} from stdin is empty."
                )));
            }
            self.stdin_read = true;
            return Ok(Some(SecretString::from(line)));
        }

        if let Some(var) = env_var {
            if var.is_empty() {
                return Err(SheafError::InvalidSelection(format!(
                    "Environment variable name for {desc} cannot be empty."
                )));
            }
            match std::env::var(var) {
                Ok(val) => {
                    if val.is_empty() {
                        return Err(SheafError::InvalidSelection(format!(
                            "Environment variable '{var}' is empty."
                        )));
                    }
                    return Ok(Some(SecretString::from(val)));
                }
                Err(_) => {
                    return Err(SheafError::InvalidSelection(format!(
                        "Environment variable '{var}' is not set."
                    )));
                }
            }
        }

        if let Some(path) = file_path {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = std::fs::metadata(path) {
                    let mode = meta.permissions().mode();
                    if mode & 0o077 != 0 {
                        let warn = format!(
                            "Password file '{}' has loose permissions (mode 0o{:o}). Restricting to 0o600 is recommended.",
                            path.display(),
                            mode & 0o777
                        );
                        eprintln!("[warning] {warn}");
                        self.warnings.push(warn);
                    }
                }
            }
            let mut content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(e) => return Err(SheafError::Io(e)),
            };
            if content.ends_with("\r\n") {
                content.truncate(content.len() - 2);
            } else if content.ends_with('\n') {
                content.truncate(content.len() - 1);
            }
            if content.is_empty() {
                return Err(SheafError::InvalidSelection(format!(
                    "Password file '{}' is empty.",
                    path.display()
                )));
            }
            return Ok(Some(SecretString::from(content)));
        }

        Ok(None)
    }
}
