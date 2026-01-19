use std::{fs, path::PathBuf};

use common::prelude::SecretKey;
use serde::{Deserialize, Serialize};

pub const APP_NAME: &str = "jax";
pub const CONFIG_FILE_NAME: &str = "config.toml";
pub const DB_FILE_NAME: &str = "db.sqlite";
pub const KEY_FILE_NAME: &str = "key.pem";
pub const BLOBS_DIR_NAME: &str = "blobs";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Port for the App server (UI + API combined)
    #[serde(default = "default_app_port")]
    pub app_port: u16,
    /// Listen port for the peer (P2P) node (optional, defaults to ephemeral)
    #[serde(default)]
    pub peer_port: Option<u16>,
    /// Listen port for the gateway server
    #[serde(default = "default_gateway_port")]
    pub gateway_port: u16,
}

fn default_app_port() -> u16 {
    8080
}

fn default_gateway_port() -> u16 {
    9090
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            app_port: default_app_port(),
            peer_port: None,
            gateway_port: default_gateway_port(),
        }
    }
}

/// Configuration for the blob storage backend.
/// This determines where blob data is stored (legacy iroh, local filesystem, or S3).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlobStoreConfig {
    /// Legacy iroh FsStore (default, for backwards compatibility)
    #[default]
    Legacy,

    /// New SQLite + local filesystem backend
    Filesystem {
        /// Path for blob storage (defaults to jax_dir/blobs-store/)
        path: Option<PathBuf>,
    },

    /// S3-compatible object storage
    S3 {
        /// S3 endpoint URL (e.g., "http://localhost:9000" for MinIO)
        endpoint: String,
        /// Access key ID
        access_key: String,
        /// Secret access key
        secret_key: String,
        /// Bucket name
        bucket: String,
        /// Optional region (defaults to "us-east-1")
        region: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct AppState {
    /// Path to the jax directory (~/.jax)
    pub jax_dir: PathBuf,
    /// Path to the SQLite database
    pub db_path: PathBuf,
    /// Path to the node key PEM file
    pub key_path: PathBuf,
    /// Path to the blobs directory
    pub blobs_path: PathBuf,
    /// Path to the config file
    pub config_path: PathBuf,
    /// Loaded configuration
    pub config: AppConfig,
}

impl AppState {
    /// Get the jax directory path (custom or default ~/.jax)
    pub fn jax_dir(custom_path: Option<PathBuf>) -> Result<PathBuf, StateError> {
        if let Some(path) = custom_path {
            return Ok(path);
        }

        // Use home directory directly since we want ~/.jax
        let home = dirs::home_dir().ok_or(StateError::NoHomeDirectory)?;
        Ok(home.join(format!(".{}", APP_NAME)))
    }

    /// Check if jax directory exists
    #[allow(dead_code)]
    pub fn exists(custom_path: Option<PathBuf>) -> Result<bool, StateError> {
        let jax_dir = Self::jax_dir(custom_path)?;
        Ok(jax_dir.exists())
    }

    /// Initialize a new jax state directory
    pub fn init(
        custom_path: Option<PathBuf>,
        config: Option<AppConfig>,
    ) -> Result<Self, StateError> {
        let jax_dir = Self::jax_dir(custom_path)?;

        // Create jax directory if it doesn't exist
        if jax_dir.exists() {
            return Err(StateError::AlreadyInitialized);
        }

        fs::create_dir_all(&jax_dir)?;

        // Create subdirectories
        let blobs_path = jax_dir.join(BLOBS_DIR_NAME);
        fs::create_dir_all(&blobs_path)?;

        // Generate and save key
        let key = SecretKey::generate();
        let key_path = jax_dir.join(KEY_FILE_NAME);
        fs::write(&key_path, key.to_pem())?;

        // Create config (use provided or default)
        let config = config.unwrap_or_default();
        let config_path = jax_dir.join(CONFIG_FILE_NAME);
        let config_toml = toml::to_string_pretty(&config)?;
        fs::write(&config_path, config_toml)?;

        // Create empty database (just touch the file, it will be initialized by the service)
        let db_path = jax_dir.join(DB_FILE_NAME);
        fs::write(&db_path, "")?;

        Ok(Self {
            jax_dir,
            db_path,
            key_path,
            blobs_path,
            config_path,
            config,
        })
    }

    /// Load existing state from jax directory
    pub fn load(custom_path: Option<PathBuf>) -> Result<Self, StateError> {
        let jax_dir = Self::jax_dir(custom_path)?;

        if !jax_dir.exists() {
            return Err(StateError::NotInitialized);
        }

        // Load paths
        let db_path = jax_dir.join(DB_FILE_NAME);
        let key_path = jax_dir.join(KEY_FILE_NAME);
        let blobs_path = jax_dir.join(BLOBS_DIR_NAME);
        let config_path = jax_dir.join(CONFIG_FILE_NAME);

        // Verify all required files/directories exist
        if !db_path.exists() {
            return Err(StateError::MissingFile("db.sqlite".to_string()));
        }
        if !key_path.exists() {
            return Err(StateError::MissingFile("key.pem".to_string()));
        }
        if !blobs_path.exists() {
            return Err(StateError::MissingFile("blobs/".to_string()));
        }
        if !config_path.exists() {
            return Err(StateError::MissingFile("config.toml".to_string()));
        }

        // Load config
        let config_toml = fs::read_to_string(&config_path)?;
        let config: AppConfig = toml::from_str(&config_toml)?;

        Ok(Self {
            jax_dir,
            db_path,
            key_path,
            blobs_path,
            config_path,
            config,
        })
    }

    /// Load the secret key from the key file
    pub fn load_key(&self) -> Result<SecretKey, StateError> {
        let pem = fs::read_to_string(&self.key_path)?;
        let key = SecretKey::from_pem(&pem).map_err(|e| StateError::InvalidKey(e.to_string()))?;
        Ok(key)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("jax directory not initialized. Run 'cli init' first")]
    NotInitialized,

    #[error("jax directory already initialized")]
    AlreadyInitialized,

    #[error("no home directory found")]
    NoHomeDirectory,

    #[error("missing required file: {0}")]
    MissingFile(String),

    #[error("invalid key: {0}")]
    InvalidKey(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),

    #[error("TOML deserialization error: {0}")]
    TomlDe(#[from] toml::de::Error),
}
