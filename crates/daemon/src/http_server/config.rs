use std::net::SocketAddr;

use url::Url;

#[derive(Debug, Clone)]
pub struct Config {
    // Listen address
    pub listen_addr: SocketAddr,
    // TODO (amiller68): at some point we will use this to
    //  support showings links to objects within a bucket
    // Host name for generating content URLs
    #[allow(unused)]
    pub hostname: Url,
    // log level for http tracing
    pub log_level: tracing::Level,
    // External gateway URL for generating share/download links
    #[allow(unused)]
    pub gateway_url: Option<String>,
}

impl Config {
    pub fn new(listen_addr: SocketAddr, gateway_url: Option<String>) -> Self {
        let hostname = Url::parse(&format!("http://localhost:{}", listen_addr.port())).unwrap();
        tracing::info!(
            "Creating HTTP server Config: listen_addr={}, gateway_url={:?}",
            listen_addr,
            gateway_url
        );
        Self {
            listen_addr,
            hostname,
            log_level: tracing::Level::INFO,
            gateway_url,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid URL: {0}")]
    Url(#[from] url::ParseError),
    #[error("Invalid Socket Address: {0}")]
    ListenAddr(#[from] std::net::AddrParseError),
}
