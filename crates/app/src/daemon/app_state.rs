//! Application state management for JAX config directories
//!
//! This module handles loading and managing JAX configuration from
//! config directories (like ~/.jax or custom paths). It's used by both
//! the CLI and Tauri desktop app.

use std::{fs, path::PathBuf};

use common::prelude::SecretKey;
use serde::{Deserialize, Serialize};

pub const APP_NAME: &str = "jax";
pub const CONFIG_FILE_NAME: &str = "config.toml";
pub const DB_FILE_NAME: &str = "db.sqlite";
pub const KEY_FILE_NAME: &str = "key.pem";
pub const BLOBS_DIR_NAME: &str = "blobs";

/// Configuration stored in config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Listen address for the HTML server
    pub html_listen_addr: String,
    /// Listen address for the API server
    pub api_listen_addr: String,
    /// Listen port for the peer (P2P) node (optional, defaults to ephemeral)
    #[serde(default)]
    pub peer_port: Option<u16>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            html_listen_addr: "0.0.0.0:8080".to_string(),
            api_listen_addr: "0.0.0.0:3000".to_string(),
            peer_port: None,
        }
    }
}

/// Application state representing a JAX config directory
#[derive(Debug, Clone)]
pub struct AppState {
    /// Path to the jax directory (~/.jax or custom)
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
    pub fn jax_dir(custom_path: Option<PathBuf>) -> Result<PathBuf, AppStateError> {
        if let Some(path) = custom_path {
            return Ok(path);
        }

        // Use home directory directly since we want ~/.jax
        let home = dirs::home_dir().ok_or(AppStateError::NoHomeDirectory)?;
        Ok(home.join(format!(".{}", APP_NAME)))
    }

    /// Check if jax directory exists
    #[allow(dead_code)]
    pub fn exists(custom_path: Option<PathBuf>) -> Result<bool, AppStateError> {
        let jax_dir = Self::jax_dir(custom_path)?;
        Ok(jax_dir.exists())
    }

    /// Initialize a new jax state directory
    pub fn init(
        custom_path: Option<PathBuf>,
        config: Option<AppConfig>,
    ) -> Result<Self, AppStateError> {
        let jax_dir = Self::jax_dir(custom_path)?;

        // Create jax directory if it doesn't exist
        if jax_dir.exists() {
            return Err(AppStateError::AlreadyInitialized);
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
    pub fn load(custom_path: Option<PathBuf>) -> Result<Self, AppStateError> {
        let jax_dir = Self::jax_dir(custom_path)?;

        if !jax_dir.exists() {
            return Err(AppStateError::NotInitialized);
        }

        // Load paths
        let db_path = jax_dir.join(DB_FILE_NAME);
        let key_path = jax_dir.join(KEY_FILE_NAME);
        let blobs_path = jax_dir.join(BLOBS_DIR_NAME);
        let config_path = jax_dir.join(CONFIG_FILE_NAME);

        // Verify all required files/directories exist
        if !db_path.exists() {
            return Err(AppStateError::MissingFile("db.sqlite".to_string()));
        }
        if !key_path.exists() {
            return Err(AppStateError::MissingFile("key.pem".to_string()));
        }
        if !blobs_path.exists() {
            return Err(AppStateError::MissingFile("blobs/".to_string()));
        }
        if !config_path.exists() {
            return Err(AppStateError::MissingFile("config.toml".to_string()));
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

    /// Load or initialize state from jax directory
    /// If not initialized, creates a new state with default or provided config
    pub fn load_or_init(
        custom_path: Option<PathBuf>,
        default_config: Option<AppConfig>,
    ) -> Result<Self, AppStateError> {
        match Self::load(custom_path.clone()) {
            Ok(state) => Ok(state),
            Err(AppStateError::NotInitialized) => Self::init(custom_path, default_config),
            Err(e) => Err(e),
        }
    }

    /// Load the secret key from the key file
    pub fn load_key(&self) -> Result<SecretKey, AppStateError> {
        let pem = fs::read_to_string(&self.key_path)?;
        let key =
            SecretKey::from_pem(&pem).map_err(|e| AppStateError::InvalidKey(e.to_string()))?;
        Ok(key)
    }

    /// Convert to ServiceConfig for daemon
    pub fn to_service_config(&self, ui_read_only: bool) -> Result<super::ServiceConfig, AppStateError> {
        let secret_key = self.load_key()?;

        // Build node listen address from peer_port if configured
        let node_listen_addr = self.config.peer_port.map(|port| {
            format!("0.0.0.0:{}", port)
                .parse()
                .expect("Failed to parse peer listen address")
        });

        Ok(super::ServiceConfig {
            node_listen_addr,
            node_secret: Some(secret_key),
            node_blobs_store_path: Some(self.blobs_path.clone()),
            html_listen_addr: self.config.html_listen_addr.parse().ok(),
            api_listen_addr: self.config.api_listen_addr.parse().ok(),
            sqlite_path: Some(self.db_path.clone()),
            log_level: tracing::Level::INFO,
            ui_read_only,
            api_hostname: None,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AppStateError {
    #[error("jax directory not initialized. Run 'jax init' first or use --config-path")]
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
