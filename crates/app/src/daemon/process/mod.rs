mod utils;

use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use futures::future::join_all;
use tokio::time::timeout;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

const FINAL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

use crate::daemon::http_server;
use crate::daemon::{ServiceConfig, ServiceState};

pub async fn spawn_service(service_config: &ServiceConfig) {
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

    // Get listen addresses from config
    let html_listen_addr = service_config
        .html_listen_addr
        .unwrap_or_else(|| SocketAddr::from_str("0.0.0.0:8080").unwrap());
    let api_listen_addr = service_config
        .api_listen_addr
        .unwrap_or_else(|| SocketAddr::from_str("0.0.0.0:5001").unwrap());

    // Spawn peer router
    let peer = state.peer().clone();
    let peer_rx = shutdown_rx.clone();
    let peer_handle = tokio::spawn(async move {
        if let Err(e) = common::peer::spawn(peer, peer_rx).await {
            tracing::error!("Peer error: {}", e);
        }
    });
    handles.push(peer_handle);

    // Arc the state for sharing with servers
    let state = std::sync::Arc::new(state);

    // Start HTML server
    let html_state = state.as_ref().clone();
    let api_url = service_config
        .api_hostname
        .clone()
        .unwrap_or_else(|| format!("http://localhost:{}", api_listen_addr.port()));
    tracing::info!("HTML server will use API URL: {}", api_url);
    let html_config =
        http_server::Config::new(html_listen_addr, Some(api_url), service_config.ui_read_only);
    let html_rx = shutdown_rx.clone();
    let html_handle = tokio::spawn(async move {
        tracing::info!("Starting HTML server on {}", html_listen_addr);
        if let Err(e) = http_server::run_html(html_config, html_state, html_rx).await {
            tracing::error!("HTML server error: {}", e);
        }
    });
    handles.push(html_handle);

    // Start API server
    let api_state = state.as_ref().clone();
    let api_config = http_server::Config::new(api_listen_addr, None, false);
    let api_rx = shutdown_rx.clone();
    let api_handle = tokio::spawn(async move {
        tracing::info!("Starting API server on {}", api_listen_addr);
        if let Err(e) = http_server::run_api(api_config, api_state, api_rx).await {
            tracing::error!("API server error: {}", e);
        }
    });
    handles.push(api_handle);

    // Start auto-mount configured FUSE mounts (non-blocking)
    let mount_manager = state.mount_manager().clone();
    tokio::spawn(async move {
        tracing::info!("Starting auto-mount of configured FUSE mounts...");
        mount_manager.start_auto_mounts().await;
    });

    let _ = graceful_waiter.await;

    // Graceful shutdown: stop all running FUSE mounts
    tracing::info!("Stopping all FUSE mounts...");
    state.mount_manager().stop_all_mounts().await;

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
