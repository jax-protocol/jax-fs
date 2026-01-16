//! JaxBucket Gateway - Read-only gateway for serving published bucket content
//!
//! The gateway runs as a mirror peer and can only access published buckets.
//! It provides HTTP endpoints for serving bucket content via the gateway route.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use anyhow::Result;
use axum::routing::get;
use axum::Router;
use clap::Parser;
use http::header::{ACCEPT, ORIGIN};
use http::Method;
use tokio::sync::watch;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

use service::{Config, ServiceState};

const FINAL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

/// JaxBucket Gateway - Read-only gateway for serving published bucket content
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port to listen on for HTTP requests
    #[arg(short, long, default_value = "8080")]
    port: u16,

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

    tracing::info!("Starting JaxBucket Gateway");

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

    // Gateway is always read-only
    config.ui_read_only = true;

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

    // Spawn peer
    let peer = state.peer().clone();
    let peer_rx = shutdown_rx.clone();
    let peer_handle = tokio::spawn(async move {
        if let Err(e) = common::peer::spawn(peer, peer_rx).await {
            tracing::error!("Peer error: {}", e);
        }
    });

    // Build gateway router
    let listen_addr = SocketAddr::from_str(&format!("0.0.0.0:{}", args.port))?;
    let router = build_gateway_router(state);

    tracing::info!("Gateway listening on {}", listen_addr);
    let listener = tokio::net::TcpListener::bind(listen_addr).await?;

    let mut server_rx = shutdown_rx.clone();
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let _ = server_rx.changed().await;
        })
        .await?;

    // Wait for peer to shut down
    let _ = tokio::time::timeout(FINAL_SHUTDOWN_TIMEOUT, peer_handle).await;

    tracing::info!("Gateway shutdown complete");
    Ok(())
}

/// Build the gateway router with gateway and health routes
fn build_gateway_router(state: ServiceState) -> Router {
    let cors_layer = CorsLayer::new()
        .allow_methods(vec![Method::GET])
        .allow_headers(vec![ACCEPT, ORIGIN])
        .allow_origin(Any)
        .allow_credentials(false);

    let trace_layer = TraceLayer::new_for_http();

    Router::new()
        // Gateway route for serving bucket content
        .route("/gw/:bucket_id/*file_path", get(service::http::html::gateway::handler))
        // Health check routes
        .nest("/_status", service::http::health::router(state.clone()))
        .with_state(state)
        .layer(cors_layer)
        .layer(trace_layer)
}
