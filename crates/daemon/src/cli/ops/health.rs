use std::fmt;
use std::path::PathBuf;

use clap::Args;
use owo_colors::OwoColorize;

use crate::cli::ui;
use jax_daemon::state::AppState;

#[derive(Args, Debug, Clone)]
pub struct Health;

#[derive(Debug)]
pub struct ConfigInfo {
    pub directory: PathBuf,
    pub api_port: u16,
    pub gateway_port: u16,
}

#[derive(Debug)]
pub enum EndpointStatus {
    Ok,
    Unhealthy(String),
    NotReachable,
}

impl fmt::Display for EndpointStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EndpointStatus::Ok => write!(f, "{}", ui::success(ui::SUCCESS, "OK")),
            EndpointStatus::Unhealthy(code) => {
                write!(f, "{}", ui::failure("UNHEALTHY", &format!("({code})")))
            }
            EndpointStatus::NotReachable => {
                write!(f, "{}", ui::failure("NOT REACHABLE", ""))
            }
        }
    }
}

#[derive(Debug)]
pub struct DaemonInfo {
    pub url: String,
    pub livez: EndpointStatus,
    pub readyz: EndpointStatus,
}

#[derive(Debug)]
pub struct HealthOutput {
    pub config: Option<ConfigInfo>,
    pub config_error: Option<String>,
    pub daemon: DaemonInfo,
}

impl fmt::Display for HealthOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}:", "Config".bold())?;
        match &self.config {
            Some(info) => {
                writeln!(f, "{}", ui::label("directory", &info.directory.display()))?;
                for name in ["config.toml", "db.sqlite", "key.pem", "blobs/"] {
                    writeln!(f, "{}", ui::label(name, &"OK".green()))?;
                }
                writeln!(f, "{}", ui::label("api_port", &info.api_port))?;
                writeln!(f, "{}", ui::label("gateway_port", &info.gateway_port))?;
            }
            None => {
                if let Some(err) = &self.config_error {
                    writeln!(f, "{}", ui::failure("error:", err))?;
                }
            }
        }

        writeln!(f)?;
        writeln!(f, "{} ({}):", "Daemon".bold(), self.daemon.url)?;
        writeln!(f, "{}", ui::label("livez", &self.daemon.livez))?;
        write!(f, "{}", ui::label("readyz", &self.daemon.readyz))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HealthError {
    #[error("Health check failed: {0}")]
    Failed(String),
}

#[async_trait::async_trait]
impl crate::cli::op::Op for Health {
    type Error = HealthError;
    type Output = HealthOutput;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        let (config, config_error) = match AppState::load(ctx.config_path.clone()) {
            Ok(state) => (
                Some(ConfigInfo {
                    directory: state.jax_dir,
                    api_port: state.config.api_port,
                    gateway_port: state.config.gateway_port,
                }),
                None,
            ),
            Err(e) => (None, Some(e.to_string())),
        };

        let base = ctx.client.base_url();
        let client = ctx.client.http_client();

        let livez_url = format!("{}/_status/livez", base.as_str().trim_end_matches('/'));
        let livez = match client.get(&livez_url).send().await {
            Ok(resp) if resp.status().is_success() => EndpointStatus::Ok,
            Ok(resp) => EndpointStatus::Unhealthy(resp.status().to_string()),
            Err(_) => EndpointStatus::NotReachable,
        };

        let readyz_url = format!("{}/_status/readyz", base.as_str().trim_end_matches('/'));
        let readyz = match client.get(&readyz_url).send().await {
            Ok(resp) if resp.status().is_success() => EndpointStatus::Ok,
            Ok(resp) => EndpointStatus::Unhealthy(resp.status().to_string()),
            Err(_) => EndpointStatus::NotReachable,
        };

        Ok(HealthOutput {
            config,
            config_error,
            daemon: DaemonInfo {
                url: base.to_string(),
                livez,
                readyz,
            },
        })
    }
}
