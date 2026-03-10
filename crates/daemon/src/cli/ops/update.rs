use std::fmt;
use std::io::{self, BufRead, Write};
use std::process::{Command, Stdio};

use clap::Args;
use indicatif::ProgressBar;

use crate::cli::ui;

const GITHUB_REPO: &str = "jax-protocol/jax-fs";
const INSTALL_SCRIPT_URL: &str =
    "https://raw.githubusercontent.com/jax-protocol/jax-fs/main/install.sh";

/// Update jax to the latest release.
#[derive(Args, Debug, Clone)]
pub struct Update {
    /// Force update even if already on latest version
    #[arg(long)]
    force: bool,

    /// Install FUSE variant (macOS Apple Silicon only)
    #[arg(long)]
    fuse: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallMethod {
    /// Installed via install script to ~/.local/bin
    Script,
    /// Installed via cargo install to ~/.cargo/bin
    Cargo,
    /// Running from a source/target build directory
    Source,
    /// Unknown installation method
    Unknown,
}

impl InstallMethod {
    fn description(&self) -> &str {
        match self {
            InstallMethod::Script => "install script (~/.local/bin)",
            InstallMethod::Cargo => "cargo install (~/.cargo/bin)",
            InstallMethod::Source => "source build (target/)",
            InstallMethod::Unknown => "unknown",
        }
    }
}

#[derive(Debug)]
pub struct UpdateOutput {
    pub current_version: String,
    pub install_method: InstallMethod,
    pub fuse_enabled: bool,
    pub latest_version: String,
    pub action: UpdateAction,
}

#[derive(Debug)]
pub enum UpdateAction {
    AlreadyUpToDate,
    Updated,
    Cancelled,
    UnknownMethod,
}

impl fmt::Display for UpdateOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.action {
            UpdateAction::AlreadyUpToDate => {
                write!(
                    f,
                    "{}",
                    ui::success(
                        "Already up to date",
                        &format!("(v{})", self.current_version)
                    )
                )
            }
            UpdateAction::Updated => {
                writeln!(f, "{}", ui::label("Current", &self.current_version))?;
                writeln!(f, "{}", ui::label("Latest", &self.latest_version))?;
                writeln!(
                    f,
                    "{}",
                    ui::label("Install", &self.install_method.description())
                )?;
                writeln!(
                    f,
                    "{}",
                    ui::label(
                        "FUSE",
                        &if self.fuse_enabled {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    )
                )?;
                writeln!(f)?;
                write!(
                    f,
                    "{}",
                    ui::success(
                        "Updated",
                        &format!(
                            "jax {} {} {}",
                            self.current_version,
                            ui::PROGRESS,
                            self.latest_version
                        )
                    )
                )
            }
            UpdateAction::Cancelled => {
                writeln!(f, "{}", ui::label("Current", &self.current_version))?;
                writeln!(f, "{}", ui::label("Latest", &self.latest_version))?;
                writeln!(
                    f,
                    "{}",
                    ui::label("Install", &self.install_method.description())
                )?;
                writeln!(
                    f,
                    "{}",
                    ui::label(
                        "FUSE",
                        &if self.fuse_enabled {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    )
                )?;
                writeln!(f)?;
                write!(f, "{}", ui::warning("Update cancelled"))
            }
            UpdateAction::UnknownMethod => {
                writeln!(f, "{}", ui::label("Current", &self.current_version))?;
                writeln!(f, "{}", ui::label("Latest", &self.latest_version))?;
                writeln!(
                    f,
                    "{}",
                    ui::label("Install", &self.install_method.description())
                )?;
                writeln!(f)?;
                writeln!(f, "{}", ui::warning("Could not detect installation method"))?;
                writeln!(f)?;
                writeln!(f, "To install via script (recommended):")?;
                write!(f, "  curl -fsSL {} | sh", INSTALL_SCRIPT_URL)
            }
        }
    }
}

fn detect_installation() -> InstallMethod {
    let Ok(exe_path) = std::env::current_exe() else {
        return InstallMethod::Unknown;
    };
    let path_str = exe_path.to_string_lossy();

    if path_str.contains("/.local/bin/") {
        InstallMethod::Script
    } else if path_str.contains("/.cargo/bin/") {
        InstallMethod::Cargo
    } else if path_str.contains("/target/") {
        InstallMethod::Source
    } else {
        InstallMethod::Unknown
    }
}

fn has_fuse_feature() -> bool {
    cfg!(feature = "fuse")
}

fn prompt_confirm(message: &str, default_yes: bool) -> bool {
    let suffix = if default_yes { "[Y/n]" } else { "[y/N]" };
    eprint!("{} {} ", message, suffix);
    let _ = io::stderr().flush();

    let stdin = io::stdin();
    let mut line = String::new();
    if stdin.lock().read_line(&mut line).is_err() {
        return default_yes;
    }

    let answer = line.trim().to_lowercase();
    if answer.is_empty() {
        default_yes
    } else {
        answer == "y" || answer == "yes"
    }
}

fn is_newer_version(current: &str, latest: &str) -> bool {
    let parse = |v: &str| -> (u32, u32, u32) {
        let parts: Vec<u32> = v
            .trim_start_matches('v')
            .split('.')
            .filter_map(|p| p.parse().ok())
            .collect();
        (
            *parts.first().unwrap_or(&0),
            *parts.get(1).unwrap_or(&0),
            *parts.get(2).unwrap_or(&0),
        )
    };
    parse(latest) > parse(current)
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("{0}")]
    Failed(String),
}

#[async_trait::async_trait]
impl crate::cli::op::Op for Update {
    type Error = UpdateError;
    type Output = UpdateOutput;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        let install_method = detect_installation();
        let current_version = env!("CARGO_PKG_VERSION").to_string();
        let fuse_enabled = has_fuse_feature();

        // Fetch latest version with spinner
        let spinner = ctx
            .progress
            .add(ProgressBar::new_spinner().with_message("Checking for updates..."));
        spinner.enable_steady_tick(std::time::Duration::from_millis(80));

        let latest_version = fetch_latest_version().await.map_err(|e| {
            spinner.finish_and_clear();
            UpdateError::Failed(format!("Failed to check for updates: {}", e))
        })?;

        spinner.finish_and_clear();

        let needs_update = is_newer_version(&current_version, &latest_version);

        if !needs_update && !self.force {
            return Ok(UpdateOutput {
                current_version,
                install_method,
                fuse_enabled,
                latest_version,
                action: UpdateAction::AlreadyUpToDate,
            });
        }

        match install_method {
            InstallMethod::Script => {
                run_install_script(self.fuse || fuse_enabled)
                    .map_err(|e| UpdateError::Failed(e.to_string()))?;

                Ok(UpdateOutput {
                    current_version,
                    install_method: InstallMethod::Script,
                    fuse_enabled,
                    latest_version,
                    action: UpdateAction::Updated,
                })
            }
            InstallMethod::Cargo | InstallMethod::Source => {
                eprintln!();
                eprintln!("You are running a local build.");

                if prompt_confirm(
                    "Switch to release binary and install to ~/.local/bin?",
                    true,
                ) {
                    let method = install_method.clone();
                    run_install_script(self.fuse || fuse_enabled)
                        .map_err(|e| UpdateError::Failed(e.to_string()))?;

                    Ok(UpdateOutput {
                        current_version,
                        install_method: method,
                        fuse_enabled,
                        latest_version,
                        action: UpdateAction::Updated,
                    })
                } else {
                    eprintln!();
                    eprintln!("To update manually:");
                    eprintln!(
                        "  cargo install --git https://github.com/{} jax-daemon",
                        GITHUB_REPO
                    );

                    Ok(UpdateOutput {
                        current_version,
                        install_method,
                        fuse_enabled,
                        latest_version,
                        action: UpdateAction::Cancelled,
                    })
                }
            }
            InstallMethod::Unknown => Ok(UpdateOutput {
                current_version,
                install_method,
                fuse_enabled,
                latest_version,
                action: UpdateAction::UnknownMethod,
            }),
        }
    }
}

async fn fetch_latest_version() -> Result<String, String> {
    let url = format!("https://api.github.com/repos/{}/releases", GITHUB_REPO);

    let output = Command::new("curl")
        .args(["-fsSL", &url])
        .output()
        .map_err(|e| format!("Failed to run curl: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to fetch releases: {}", stderr));
    }

    let body = String::from_utf8_lossy(&output.stdout);

    // Find first jax-daemon-v* tag
    for line in body.lines() {
        let line = line.trim();
        if line.contains("\"tag_name\"") && line.contains("jax-daemon-v") {
            if let Some(start) = line.find("jax-daemon-v") {
                let rest = &line[start + "jax-daemon-v".len()..];
                if let Some(end) = rest.find('"') {
                    return Ok(rest[..end].to_string());
                }
            }
        }
    }

    Err("Could not find jax-daemon release version".to_string())
}

fn run_install_script(fuse: bool) -> Result<(), String> {
    eprintln!();
    eprintln!("Running install script...");
    eprintln!();

    let fuse_flag = if fuse { " --fuse" } else { "" };
    let cmd = format!("curl -fsSL {} | sh -s --{}", INSTALL_SCRIPT_URL, fuse_flag);

    let status = Command::new("sh")
        .args(["-c", &cmd])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("Failed to run install script: {}", e))?;

    if !status.success() {
        return Err("Install script failed".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("0.1.0", "0.1.1"));
        assert!(is_newer_version("0.1.9", "0.2.0"));
        assert!(is_newer_version("0.1.9", "1.0.0"));
        assert!(!is_newer_version("0.1.9", "0.1.9"));
        assert!(!is_newer_version("0.2.0", "0.1.9"));
    }

    #[test]
    fn test_has_fuse_feature() {
        // Just verify it compiles and returns a bool
        let _ = has_fuse_feature();
    }

    #[test]
    fn test_detect_installation() {
        // Should return some variant without panicking
        let _ = detect_installation();
    }

    #[test]
    fn test_update_output_already_up_to_date() {
        let output = UpdateOutput {
            current_version: "0.1.0".to_string(),
            install_method: InstallMethod::Script,
            fuse_enabled: false,
            latest_version: "0.1.0".to_string(),
            action: UpdateAction::AlreadyUpToDate,
        };
        let text = format!("{output}");
        assert!(text.contains("Already up to date"));
        assert!(text.contains("0.1.0"));
    }

    #[test]
    fn test_update_output_updated() {
        let output = UpdateOutput {
            current_version: "0.1.0".to_string(),
            install_method: InstallMethod::Script,
            fuse_enabled: false,
            latest_version: "0.2.0".to_string(),
            action: UpdateAction::Updated,
        };
        let text = format!("{output}");
        assert!(text.contains("Updated"));
        assert!(text.contains("0.1.0"));
        assert!(text.contains("0.2.0"));
    }

    #[test]
    fn test_update_output_cancelled() {
        let output = UpdateOutput {
            current_version: "0.1.0".to_string(),
            install_method: InstallMethod::Cargo,
            fuse_enabled: false,
            latest_version: "0.2.0".to_string(),
            action: UpdateAction::Cancelled,
        };
        let text = format!("{output}");
        assert!(text.contains("Update cancelled"));
    }

    #[test]
    fn test_update_output_unknown_method() {
        let output = UpdateOutput {
            current_version: "0.1.0".to_string(),
            install_method: InstallMethod::Unknown,
            fuse_enabled: false,
            latest_version: "0.2.0".to_string(),
            action: UpdateAction::UnknownMethod,
        };
        let text = format!("{output}");
        assert!(text.contains("Could not detect installation method"));
        assert!(text.contains("curl"));
    }
}
