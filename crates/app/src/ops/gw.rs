use clap::Args;

use crate::daemon::gateway_process::spawn_gateway_service;
use crate::daemon::ServiceConfig;
use crate::state::AppState;

#[derive(Args, Debug, Clone)]
pub struct Gw {
    /// Port for gateway HTTP server (default: 8080)
    #[arg(long, default_value = "8080")]
    pub port: u16,
}

#[derive(Debug, thiserror::Error)]
pub enum GwError {
    #[error("state error: {0}")]
    StateError(#[from] crate::state::StateError),

    #[error("gateway failed: {0}")]
    Failed(String),
}

#[async_trait::async_trait]
impl crate::op::Op for Gw {
    type Error = GwError;
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

        // Build gateway config - only gateway port matters, no HTML/API servers
        let config = ServiceConfig {
            node_listen_addr,
            node_secret: Some(secret_key),
            node_blobs_store_path: Some(state.blobs_path),
            // Gateway listens on specified port
            html_listen_addr: Some(
                format!("0.0.0.0:{}", self.port)
                    .parse()
                    .expect("Failed to parse gateway listen address"),
            ),
            // API server not used in gateway mode
            api_listen_addr: None,
            sqlite_path: Some(state.db_path),
            log_level: tracing::Level::DEBUG,
            // UI settings not used in gateway mode
            ui_read_only: true,
            api_hostname: None,
            // Gateway settings not used in standalone gateway mode
            gateway_port: None,
            gateway_url: None,
        };

        spawn_gateway_service(&config).await;
        Ok("gateway ended".to_string())
    }
}
