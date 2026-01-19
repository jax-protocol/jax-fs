use std::net::SocketAddr;
use std::path::PathBuf;

use common::prelude::SecretKey;

#[derive(Debug)]
pub struct Config {
    // peer configuration
    /// address for our jax peer to listen on,
    ///  if not set then an ephemeral port will be used
    pub node_listen_addr: Option<SocketAddr>,
    /// on system file path to our secret,
    ///  if not set then a new secret will be generated
    pub node_secret: Option<SecretKey>,
    /// the path to our blobs store, if not set then
    ///  a temporary directory will be used
    pub node_blobs_store_path: Option<PathBuf>,

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

    // misc
    pub log_level: tracing::Level,

    // url configuration
    /// API URL for HTML UI (for JS to call API endpoints)
    /// If not set, defaults to same origin as the UI
    pub api_url: Option<String>,
    /// External gateway URL (e.g., "https://gateway.example.com")
    /// Used for generating share/download links
    pub gateway_url: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            node_listen_addr: None,
            node_secret: None,
            node_blobs_store_path: None,
            app_port: Some(8080), // Default app server on 8080
            gateway_port: None,   // No gateway by default
            sqlite_path: None,
            log_level: tracing::Level::INFO,
            api_url: None,
            gateway_url: None,
        }
    }
}

// TODO (amiller68): real error handling
#[allow(dead_code)]
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {}
