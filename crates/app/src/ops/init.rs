use std::path::PathBuf;

use clap::{Args, ValueEnum};

use crate::state::{AppConfig, AppState, BlobStoreConfig};

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

    /// Blob store backend type
    #[arg(long, value_enum, default_value_t = BlobStoreType::Legacy)]
    pub blob_store: BlobStoreType,

    /// S3/MinIO URL (required for --blob-store s3)
    /// Format: s3://access_key:secret_key@host:port/bucket
    /// Example: s3://minioadmin:minioadmin@localhost:9000/jax-blobs
    #[arg(long)]
    pub s3_url: Option<String>,

    /// Filesystem blob store path (required for --blob-store filesystem)
    /// Must be an absolute path
    #[arg(long)]
    pub blobs_path: Option<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("init failed: {0}")]
    StateFailed(#[from] crate::state::StateError),

    #[error("missing required config: {0}")]
    MissingConfig(String),

    #[error("invalid path: {0}")]
    InvalidPath(String),
}

impl Init {
    /// Build blob store configuration from CLI flags
    fn build_blob_store_config(
        &self,
        jax_dir: &std::path::Path,
    ) -> Result<BlobStoreConfig, InitError> {
        match self.blob_store {
            BlobStoreType::Legacy => Ok(BlobStoreConfig::Legacy),

            BlobStoreType::Filesystem => {
                let path = match &self.blobs_path {
                    Some(p) => {
                        if !p.is_absolute() {
                            return Err(InitError::InvalidPath(
                                "--blobs-path must be an absolute path".to_string(),
                            ));
                        }
                        p.clone()
                    }
                    None => {
                        // Default to jax_dir/blobs-store/
                        jax_dir.join("blobs-store")
                    }
                };
                Ok(BlobStoreConfig::Filesystem { path })
            }

            BlobStoreType::S3 => {
                let url = self.s3_url.clone().ok_or_else(|| {
                    InitError::MissingConfig("--s3-url required for S3 backend".to_string())
                })?;

                // Validate URL format by parsing it
                BlobStoreConfig::parse_s3_url(&url)?;

                Ok(BlobStoreConfig::S3 { url })
            }
        }
    }
}

#[async_trait::async_trait]
impl crate::op::Op for Init {
    type Error = InitError;
    type Output = String;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        // Get jax_dir path first so we can use it for blob store config
        let jax_dir = AppState::jax_dir(ctx.config_path.clone())?;

        // Build blob store configuration
        let blob_store = self.build_blob_store_config(&jax_dir)?;

        let config = AppConfig {
            app_port: self.app_port,
            peer_port: self.peer_port,
            gateway_port: self.gateway_port,
            blob_store: blob_store.clone(),
        };

        let state = AppState::init(ctx.config_path.clone(), Some(config))?;

        let peer_port_str = match state.config.peer_port {
            Some(port) => format!("{}", port),
            None => "ephemeral (auto-assigned)".to_string(),
        };

        let blob_store_str = match &state.config.blob_store {
            BlobStoreConfig::Legacy => "legacy (iroh FsStore)".to_string(),
            BlobStoreConfig::Filesystem { path } => format!("filesystem ({})", path.display()),
            BlobStoreConfig::S3 { url } => {
                // Mask credentials in output
                format!("s3 ({})", mask_s3_url(url))
            }
        };

        let output = format!(
            "Initialized jax directory at: {}\n\
             - Database: {}\n\
             - Key: {}\n\
             - Blobs: {}\n\
             - Config: {}\n\
             - App port: {}\n\
             - Peer port: {}\n\
             - Gateway port: {}\n\
             - Blob store: {}",
            state.jax_dir.display(),
            state.db_path.display(),
            state.key_path.display(),
            state.blobs_path.display(),
            state.config_path.display(),
            state.config.app_port,
            peer_port_str,
            state.config.gateway_port,
            blob_store_str
        );

        Ok(output)
    }
}

/// Mask credentials in S3 URL for display
fn mask_s3_url(url: &str) -> String {
    // s3://access_key:secret_key@host:port/bucket -> s3://***:***@host:port/bucket
    if let Some(rest) = url.strip_prefix("s3://") {
        if let Some(at_pos) = rest.find('@') {
            let host_bucket = &rest[at_pos..];
            return format!("s3://***:***{}", host_bucket);
        }
    }
    url.to_string()
}
