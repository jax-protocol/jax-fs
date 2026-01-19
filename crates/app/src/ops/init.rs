use clap::Args;

use crate::state::{AppConfig, AppState};

#[derive(Args, Debug, Clone)]
pub struct Init {
    /// App server port (UI + API combined, default: 8080)
    #[arg(long, default_value = "8080")]
    pub app_port: u16,

    /// Peer (P2P) node listen port (optional, defaults to ephemeral port if not specified)
    #[arg(long)]
    pub peer_port: Option<u16>,

    /// Gateway server listen port (default: 9090)
    #[arg(long, default_value = "9090")]
    pub gateway_port: u16,
}

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("init failed: {0}")]
    StateFailed(#[from] crate::state::StateError),
}

#[async_trait::async_trait]
impl crate::op::Op for Init {
    type Error = InitError;
    type Output = String;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        let config = AppConfig {
            app_port: self.app_port,
            peer_port: self.peer_port,
            gateway_port: self.gateway_port,
        };

        let state = AppState::init(ctx.config_path.clone(), Some(config))?;

        let peer_port_str = match state.config.peer_port {
            Some(port) => format!("{}", port),
            None => "ephemeral (auto-assigned)".to_string(),
        };

        let output = format!(
            "Initialized jax directory at: {}\n\
             - Database: {}\n\
             - Key: {}\n\
             - Blobs: {}\n\
             - Config: {}\n\
             - App port: {}\n\
             - Peer port: {}\n\
             - Gateway port: {}",
            state.jax_dir.display(),
            state.db_path.display(),
            state.key_path.display(),
            state.blobs_path.display(),
            state.config_path.display(),
            state.config.app_port,
            peer_port_str,
            state.config.gateway_port
        );

        Ok(output)
    }
}
