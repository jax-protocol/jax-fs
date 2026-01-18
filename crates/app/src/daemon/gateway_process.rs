use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use futures::future::join_all;
use tokio::time::timeout;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

use crate::daemon::http_server;
use crate::daemon::process::utils;
use crate::daemon::{ServiceConfig, ServiceState};

const FINAL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

/// Spawns a minimal gateway service with only P2P peer and gateway content serving.
/// No Askama UI routes, no REST API routes.
pub async fn spawn_gateway_service(service_config: &ServiceConfig) {
    let (non_blocking_writer, _guard) = tracing_appender::non_blocking(std::io::stdout());
    let env_filter = EnvFilter::builder()
        .with_default_directive(service_config.log_level.into())
        .from_env_lossy();

    let stderr_layer = tracing_subscriber::fmt::layer()
        .compact()
        .with_writer(non_blocking_writer)
        .with_filter(env_filter);

    tracing_subscriber::registry().with(stderr_layer).init();

    utils::register_panic_logger();
    utils::report_build_info();

    let (graceful_waiter, shutdown_rx) = utils::graceful_shutdown_blocker();

    // Create state
    let state = match ServiceState::from_config(service_config).await {
        Ok(state) => state,
        Err(e) => {
            tracing::error!("error creating server state: {}", e);
            std::process::exit(3);
        }
    };

    let mut handles = Vec::new();

    // Get gateway listen address from config (use html_listen_addr for gateway port)
    let gateway_listen_addr = service_config
        .html_listen_addr
        .unwrap_or_else(|| SocketAddr::from_str("0.0.0.0:8080").unwrap());

    // Spawn peer router
    let peer = state.peer().clone();
    let peer_rx = shutdown_rx.clone();
    let peer_handle = tokio::spawn(async move {
        if let Err(e) = common::peer::spawn(peer, peer_rx).await {
            tracing::error!("Peer error: {}", e);
        }
    });
    handles.push(peer_handle);

    // Arc the state for sharing with server
    let state = std::sync::Arc::new(state);

    // Start gateway-only HTTP server (no UI, no API)
    let gw_state = state.as_ref().clone();
    let gw_config = http_server::Config::new(gateway_listen_addr, None, true, None);
    let gw_rx = shutdown_rx.clone();
    let gw_handle = tokio::spawn(async move {
        tracing::info!("Starting gateway server on {}", gateway_listen_addr);
        if let Err(e) = http_server::run_gateway(gw_config, gw_state, gw_rx).await {
            tracing::error!("Gateway server error: {}", e);
        }
    });
    handles.push(gw_handle);

    let _ = graceful_waiter.await;

    if timeout(FINAL_SHUTDOWN_TIMEOUT, join_all(handles))
        .await
        .is_err()
    {
        tracing::error!(
            "Failed to shut down within {} seconds",
            FINAL_SHUTDOWN_TIMEOUT.as_secs()
        );
        std::process::exit(4);
    }
}
