use std::fmt;
use std::path::PathBuf;

use crate::cli::ui;
use clap::{Args, ValueEnum};
use jax_daemon::state::{AppConfig, AppState, BlobStoreConfig};

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
    /// API server port (private, mutation/RPC, default: 5001)
    #[arg(long, default_value = "5001")]
    pub api_port: u16,

    /// Gateway server port (public, read-only, default: 8080)
    #[arg(long, default_value = "8080")]
    pub gateway_port: u16,

    /// Peer (P2P) node listen port (optional, defaults to ephemeral port if not specified)
    #[arg(long)]
    pub peer_port: Option<u16>,

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

#[derive(Debug)]
pub struct InitOutput {
    pub jax_dir: PathBuf,
    pub db_path: PathBuf,
    pub key_path: PathBuf,
    pub blobs_path: PathBuf,
    pub config_path: PathBuf,
    pub api_port: u16,
    pub gateway_port: u16,
    pub peer_port: Option<u16>,
    pub blob_store: String,
}

impl fmt::Display for InitOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}",
            ui::success("Initialized", &format!("jax at {}", self.jax_dir.display()))
        )?;
        writeln!(f, "{}", ui::label("Database", &self.db_path.display()))?;
        writeln!(f, "{}", ui::label("Key", &self.key_path.display()))?;
        writeln!(f, "{}", ui::label("Blobs", &self.blobs_path.display()))?;
        writeln!(f, "{}", ui::label("Config", &self.config_path.display()))?;
        writeln!(f, "{}", ui::label("API port", &self.api_port))?;
        writeln!(f, "{}", ui::label("Gateway port", &self.gateway_port))?;
        let peer_port_str = match self.peer_port {
            Some(port) => port.to_string(),
            None => "ephemeral (auto-assigned)".to_string(),
        };
        writeln!(f, "{}", ui::label("Peer port", &peer_port_str))?;
        write!(f, "{}", ui::label("Blob store", &self.blob_store))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("init failed: {0}")]
    StateFailed(#[from] jax_daemon::state::StateError),

    #[error("missing required config: {0}")]
    MissingConfig(String),

    #[error("invalid path: {0}")]
    InvalidPath(String),
}

impl Init {
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
                    None => jax_dir.join("blobs-store"),
                };
                Ok(BlobStoreConfig::Filesystem {
                    path,
                    db_path: None,
                })
            }

            BlobStoreType::S3 => {
                let url = self.s3_url.clone().ok_or_else(|| {
                    InitError::MissingConfig("--s3-url required for S3 backend".to_string())
                })?;

                BlobStoreConfig::parse_s3_url(&url)?;

                Ok(BlobStoreConfig::S3 { url })
            }
        }
    }
}

#[async_trait::async_trait]
impl crate::cli::op::Op for Init {
    type Error = InitError;
    type Output = InitOutput;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        let jax_dir = AppState::jax_dir(ctx.config_path.clone())?;
        let blob_store = self.build_blob_store_config(&jax_dir)?;

        let config = AppConfig {
            api_port: self.api_port,
            gateway_port: self.gateway_port,
            peer_port: self.peer_port,
            blob_store: blob_store.clone(),
            ..Default::default()
        };

        let state = AppState::init(ctx.config_path.clone(), Some(config))?;

        let blob_store_str = match &state.config.blob_store {
            BlobStoreConfig::Legacy => "legacy (iroh FsStore)".to_string(),
            BlobStoreConfig::Filesystem { path, db_path } => {
                let db_info = match db_path {
                    Some(p) => format!(", db: {}", p.display()),
                    None => String::new(),
                };
                format!("filesystem ({}{})", path.display(), db_info)
            }
            BlobStoreConfig::S3 { url } => {
                // Mask credentials in output
                format!("s3 ({})", mask_s3_url(url))
            }
        };

        Ok(InitOutput {
            jax_dir: state.jax_dir,
            db_path: state.db_path,
            key_path: state.key_path,
            blobs_path: state.blobs_path,
            config_path: state.config_path,
            api_port: state.config.api_port,
            gateway_port: state.config.gateway_port,
            peer_port: state.config.peer_port,
            blob_store: blob_store_str,
        })
    }
}

fn mask_s3_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("s3://") {
        if let Some(at_pos) = rest.find('@') {
            let host_bucket = &rest[at_pos..];
            return format!("s3://***:***{}", host_bucket);
        }
    }
    url.to_string()
}
