use clap::Args;

use crate::daemon::{spawn_service, ServiceConfig};
use crate::state::AppState;

#[derive(Args, Debug, Clone)]
pub struct Daemon {
    /// Run only the gateway server (no App UI/API)
    #[arg(long)]
    pub gateway: bool,

    /// API URL for HTML UI (default: same origin)
    #[arg(long)]
    pub api_url: Option<String>,

    /// Override gateway port (implies gateway is enabled)
    #[arg(long)]
    pub gateway_port: Option<u16>,

    /// Gateway URL for share/download links (e.g., https://gateway.example.com)
    #[arg(long)]
    pub gateway_url: Option<String>,

    /// Also run gateway alongside app server
    #[arg(long)]
    pub with_gateway: bool,

    /// Directory for log files (logs to stdout only if not set)
    #[arg(long)]
    pub log_dir: Option<std::path::PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error("state error: {0}")]
    StateError(#[from] crate::state::StateError),

    #[error("daemon failed: {0}")]
    Failed(String),
}

#[async_trait::async_trait]
impl crate::op::Op for Daemon {
    type Error = DaemonError;
    type Output = String;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        // Load state from config path (or default ~/.jax)
        let state = AppState::load(ctx.config_path.clone())?;

        // Load the secret key
        let secret_key = state.load_key()?;

        // Build node listen address from peer_port if configured
        let node_listen_addr = state.config.peer_port.map(|port| {
            format!("0.0.0.0:{}", port)
                .parse()
                .expect("Failed to parse peer listen address")
        });

        // Determine app_port and gateway_port based on flags
        let (app_port, gateway_port) = if self.gateway {
            // Gateway-only mode: no app server, just gateway
            let gateway_port = self.gateway_port.unwrap_or(state.config.gateway_port);
            (None, Some(gateway_port))
        } else if self.with_gateway || self.gateway_port.is_some() {
            // App + Gateway mode: run both
            let gateway_port = self.gateway_port.unwrap_or(state.config.gateway_port);
            (Some(state.config.app_port), Some(gateway_port))
        } else {
            // Default: App only, no gateway
            (Some(state.config.app_port), None)
        };

        // Blob store configuration is read from config.toml (set at init time)
        let config = ServiceConfig {
            node_listen_addr,
            node_secret: Some(secret_key),
            blob_store: state.config.blob_store.clone(),
            jax_dir: state.jax_dir.clone(),
            app_port,
            gateway_port,
            sqlite_path: Some(state.db_path),
            log_level: tracing::Level::DEBUG,
            log_dir: self.log_dir.clone(),
            api_url: self.api_url.clone(),
            gateway_url: self.gateway_url.clone(),
        };

        spawn_service(&config).await;
        Ok("daemon ended".to_string())
    }
}
