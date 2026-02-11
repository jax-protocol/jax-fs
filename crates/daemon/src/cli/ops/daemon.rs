use clap::Args;

use jax_daemon::state::AppState;
use jax_daemon::{spawn_service, ServiceConfig};

#[derive(Args, Debug, Clone)]
pub struct Daemon {
    /// Override API server port (default from config)
    #[arg(long)]
    pub api_port: Option<u16>,

    /// Override gateway server port (default from config)
    #[arg(long)]
    pub gateway_port: Option<u16>,

    /// Gateway URL for share/download links (e.g., https://gateway.example.com)
    #[arg(long)]
    pub gateway_url: Option<String>,

    /// Directory for log files (logs to stdout only if not set)
    #[arg(long)]
    pub log_dir: Option<std::path::PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error("state error: {0}")]
    StateError(#[from] jax_daemon::state::StateError),

    #[error("daemon failed: {0}")]
    Failed(String),
}

#[async_trait::async_trait]
impl crate::cli::op::Op for Daemon {
    type Error = DaemonError;
    type Output = String;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
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

        // Use ports from flags or config
        let api_port = self.api_port.unwrap_or(state.config.api_port);
        let gateway_port = self.gateway_port.unwrap_or(state.config.gateway_port);

        // Blob store configuration is read from config.toml (set at init time)
        let config = ServiceConfig {
            node_listen_addr,
            node_secret: Some(secret_key),
            blob_store: state.config.blob_store.clone(),
            jax_dir: state.jax_dir.clone(),
            api_port,
            gateway_port,
            sqlite_path: Some(state.db_path),
            log_level: tracing::Level::DEBUG,
            log_dir: self.log_dir.clone(),
            gateway_url: self.gateway_url.clone(),
        };

        spawn_service(&config).await;
        Ok("daemon ended".to_string())
    }
}
