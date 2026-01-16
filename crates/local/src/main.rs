//! JaxBucket Local Peer - Full-featured local peer with write API
//!
//! The local peer runs as an owner peer with full read/write access.
//! It provides HTTP endpoints for both the HTML UI and JSON API.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use anyhow::Result;
use axum::extract::DefaultBodyLimit;
use axum::{Extension, Router};
use clap::Parser;
use http::header::{ACCEPT, CONTENT_TYPE, ORIGIN};
use http::Method;
use tokio::sync::watch;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

use service::{http::Config as HttpConfig, Config, ServiceState};

/// Maximum upload size in bytes (500 MB)
const MAX_UPLOAD_SIZE_BYTES: usize = 500 * 1024 * 1024;

const FINAL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

/// JaxBucket Local Peer - Full-featured local peer with write API
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port for the HTML UI server
    #[arg(long, default_value = "8080")]
    html_port: u16,

    /// Port for the API server
    #[arg(long, default_value = "3000")]
    api_port: u16,

    /// Path to SQLite database file
    #[arg(short, long)]
    database: Option<PathBuf>,

    /// Path to blobs storage directory
    #[arg(short, long)]
    blobs: Option<PathBuf>,

    /// Port for the peer to listen on (for p2p networking)
    #[arg(long)]
    peer_port: Option<u16>,

    /// Log level (error, warn, info, debug, trace)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// API hostname for HTML UI to use when making API requests
    #[arg(long)]
    api_hostname: Option<String>,

    /// Run in read-only mode (disables write operations in UI)
    #[arg(long)]
    read_only: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing
    let (non_blocking_writer, _guard) = tracing_appender::non_blocking(std::io::stdout());
    let log_level: tracing::Level = args.log_level.parse().unwrap_or(tracing::Level::INFO);
    let env_filter = EnvFilter::builder()
        .with_default_directive(log_level.into())
        .from_env_lossy();

    let stderr_layer = tracing_subscriber::fmt::layer()
        .compact()
        .with_writer(non_blocking_writer)
        .with_filter(env_filter);

    tracing_subscriber::registry().with(stderr_layer).init();

    tracing::info!("Starting JaxBucket Local Peer");

    // Create configuration
    let mut config = Config::default();

    // Set database path if provided
    if let Some(db_path) = args.database {
        config.sqlite_path = Some(db_path);
    }

    // Set blobs path if provided
    if let Some(blobs_path) = args.blobs {
        config.node_blobs_store_path = Some(blobs_path);
    }

    // Set peer listen address if provided
    if let Some(peer_port) = args.peer_port {
        config.node_listen_addr = Some(SocketAddr::from_str(&format!("0.0.0.0:{}", peer_port))?);
    }

    config.ui_read_only = args.read_only;
    config.api_hostname = args.api_hostname.clone();

    // Create state
    let state = match ServiceState::from_config(&config).await {
        Ok(state) => state,
        Err(e) => {
            tracing::error!("Failed to create service state: {}", e);
            std::process::exit(1);
        }
    };

    // Set up graceful shutdown
    let (shutdown_tx, shutdown_rx) = watch::channel(());
    let graceful_shutdown = async move {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl+c");
        tracing::info!("Received shutdown signal");
        let _ = shutdown_tx.send(());
    };
    tokio::spawn(graceful_shutdown);

    let mut handles = Vec::new();

    // Spawn peer
    let peer = state.peer().clone();
    let peer_rx = shutdown_rx.clone();
    let peer_handle = tokio::spawn(async move {
        if let Err(e) = common::peer::spawn(peer, peer_rx).await {
            tracing::error!("Peer error: {}", e);
        }
    });
    handles.push(peer_handle);

    // Start HTML server
    let html_listen_addr = SocketAddr::from_str(&format!("0.0.0.0:{}", args.html_port))?;
    let api_url = args
        .api_hostname
        .clone()
        .unwrap_or_else(|| format!("http://localhost:{}", args.api_port));

    let html_config = HttpConfig::new(html_listen_addr, Some(api_url), args.read_only);
    let html_state = state.clone();
    let html_rx = shutdown_rx.clone();

    let html_handle = tokio::spawn(async move {
        tracing::info!("Starting HTML server on {}", html_listen_addr);
        if let Err(e) = run_html_server(html_config, html_state, html_rx).await {
            tracing::error!("HTML server error: {}", e);
        }
    });
    handles.push(html_handle);

    // Start API server
    let api_listen_addr = SocketAddr::from_str(&format!("0.0.0.0:{}", args.api_port))?;
    let api_state = state.clone();
    let api_rx = shutdown_rx.clone();

    let api_handle = tokio::spawn(async move {
        tracing::info!("Starting API server on {}", api_listen_addr);
        if let Err(e) = run_api_server(api_listen_addr, api_state, api_rx).await {
            tracing::error!("API server error: {}", e);
        }
    });
    handles.push(api_handle);

    // Wait for shutdown
    let _ = shutdown_rx.clone().changed().await;

    // Wait for all handles with timeout
    let _ = tokio::time::timeout(
        FINAL_SHUTDOWN_TIMEOUT,
        futures::future::join_all(handles),
    )
    .await;

    tracing::info!("Local peer shutdown complete");
    Ok(())
}

/// Run the HTML server with full UI
async fn run_html_server(
    config: HttpConfig,
    state: ServiceState,
    mut shutdown_rx: watch::Receiver<()>,
) -> Result<()> {
    let listen_addr = config.listen_addr;
    let trace_layer = TraceLayer::new_for_http();

    let router = Router::new()
        .nest("/_status", service::http::health::router(state.clone()))
        .merge(service::http::html::router(state.clone()))
        .fallback(service::http::not_found_handler)
        .layer(Extension(config))
        .with_state(state)
        .layer(trace_layer);

    let listener = tokio::net::TcpListener::bind(listen_addr).await?;

    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.changed().await;
        })
        .await?;

    Ok(())
}

/// Run the API server with write endpoints
async fn run_api_server(
    listen_addr: SocketAddr,
    state: ServiceState,
    mut shutdown_rx: watch::Receiver<()>,
) -> Result<()> {
    let cors_layer = CorsLayer::new()
        .allow_methods(vec![Method::GET, Method::POST])
        .allow_headers(vec![ACCEPT, CONTENT_TYPE, ORIGIN])
        .allow_origin(Any)
        .allow_credentials(false);

    let trace_layer = TraceLayer::new_for_http();

    let router = Router::new()
        .nest("/_status", service::http::health::router(state.clone()))
        .nest("/api", jax_local::http::api::router(state.clone()))
        .fallback(service::http::not_found_handler)
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_SIZE_BYTES))
        .with_state(state)
        .layer(cors_layer)
        .layer(trace_layer);

    let listener = tokio::net::TcpListener::bind(listen_addr).await?;

    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.changed().await;
        })
        .await?;

    Ok(())
}
