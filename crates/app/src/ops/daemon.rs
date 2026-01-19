use clap::{Args, ValueEnum};

use crate::daemon::{spawn_service, ServiceConfig};
use crate::state::{AppState, BlobStoreConfig};

/// Blob store backend type for CLI selection
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum BlobStoreType {
    /// Legacy iroh FsStore (default)
    #[default]
    Legacy,
    /// SQLite + local filesystem
    Filesystem,
    /// S3-compatible object storage
    S3,
}

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

    // Blob store configuration
    /// Blob store backend type
    #[arg(long, value_enum, default_value_t = BlobStoreType::Legacy)]
    pub blob_store: BlobStoreType,

    /// S3 endpoint URL (required for --blob-store s3)
    #[arg(long)]
    pub s3_endpoint: Option<String>,

    /// S3 bucket name (required for --blob-store s3)
    #[arg(long)]
    pub s3_bucket: Option<String>,

    /// S3 access key (can also use JAX_S3_ACCESS_KEY env var)
    #[arg(long, env = "JAX_S3_ACCESS_KEY")]
    pub s3_access_key: Option<String>,

    /// S3 secret key (can also use JAX_S3_SECRET_KEY env var)
    #[arg(long, env = "JAX_S3_SECRET_KEY")]
    pub s3_secret_key: Option<String>,

    /// S3 region (optional, defaults to us-east-1)
    #[arg(long)]
    pub s3_region: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error("state error: {0}")]
    StateError(#[from] crate::state::StateError),

    #[error("daemon failed: {0}")]
    Failed(String),

    #[error("missing required S3 configuration: {0}")]
    MissingS3Config(String),
}

impl Daemon {
    /// Build blob store configuration from CLI flags
    fn build_blob_store_config(&self) -> Result<BlobStoreConfig, DaemonError> {
        match self.blob_store {
            BlobStoreType::Legacy => Ok(BlobStoreConfig::Legacy),

            BlobStoreType::Filesystem => Ok(BlobStoreConfig::Filesystem { path: None }),

            BlobStoreType::S3 => {
                let endpoint = self
                    .s3_endpoint
                    .clone()
                    .ok_or_else(|| DaemonError::MissingS3Config("--s3-endpoint".to_string()))?;
                let bucket = self
                    .s3_bucket
                    .clone()
                    .ok_or_else(|| DaemonError::MissingS3Config("--s3-bucket".to_string()))?;
                let access_key = self.s3_access_key.clone().ok_or_else(|| {
                    DaemonError::MissingS3Config("--s3-access-key or JAX_S3_ACCESS_KEY".to_string())
                })?;
                let secret_key = self.s3_secret_key.clone().ok_or_else(|| {
                    DaemonError::MissingS3Config("--s3-secret-key or JAX_S3_SECRET_KEY".to_string())
                })?;

                Ok(BlobStoreConfig::S3 {
                    endpoint,
                    access_key,
                    secret_key,
                    bucket,
                    region: self.s3_region.clone(),
                })
            }
        }
    }
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

        // Build blob store configuration from CLI flags
        let blob_store = self.build_blob_store_config()?;

        let config = ServiceConfig {
            node_listen_addr,
            node_secret: Some(secret_key),
            blob_store,
            jax_dir: Some(state.jax_dir),
            app_port,
            gateway_port,
            sqlite_path: Some(state.db_path),
            log_level: tracing::Level::DEBUG,
            api_url: self.api_url.clone(),
            gateway_url: self.gateway_url.clone(),
        };

        spawn_service(&config).await;
        Ok("daemon ended".to_string())
    }
}
