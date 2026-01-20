use std::net::SocketAddr;
use std::path::PathBuf;

use common::prelude::SecretKey;

use crate::state::BlobStoreConfig;

#[derive(Debug)]
pub struct Config {
    // peer configuration
    /// address for our jax peer to listen on,
    ///  if not set then an ephemeral port will be used
    pub node_listen_addr: Option<SocketAddr>,
    /// on system file path to our secret,
    ///  if not set then a new secret will be generated
    pub node_secret: Option<SecretKey>,

    // blob store configuration
    /// Blob storage backend configuration
    pub blob_store: BlobStoreConfig,
    /// Path to the jax directory (absolute path, used for legacy blobs and cache)
    pub jax_dir: PathBuf,

    // http server configuration - just two optional ports
    /// Port for the App server (UI + API combined).
    /// If not set, no app server will be started.
    pub app_port: Option<u16>,
    /// Port for the Gateway server (read-only content serving).
    /// If not set, no gateway server will be started.
    pub gateway_port: Option<u16>,

    // data store configuration
    /// a path to a sqlite database, if not set then an
    ///  in-memory database will be used
    pub sqlite_path: Option<PathBuf>,

    // logging
    pub log_level: tracing::Level,
    /// Directory for log files (optional, logs to stdout only if not set)
    pub log_dir: Option<PathBuf>,

    // url configuration
    /// API URL for HTML UI (for JS to call API endpoints)
    /// If not set, defaults to same origin as the UI
    pub api_url: Option<String>,
    /// External gateway URL (e.g., "https://gateway.example.com")
    /// Used for generating share/download links
    pub gateway_url: Option<String>,
}

// TODO (amiller68): real error handling
#[allow(dead_code)]
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {}
