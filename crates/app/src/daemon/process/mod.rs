pub mod utils;

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

/// Initialize logging, panic handler, and build info reporting.
/// Returns guards that must be kept alive for the duration of the program.
fn init_logging(
    service_config: &ServiceConfig,
) -> Vec<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::fmt::format::FmtSpan;

    let mut guards = Vec::new();

    // Stdout layer
    let (stdout_writer, stdout_guard) = tracing_appender::non_blocking(std::io::stdout());
    guards.push(stdout_guard);

    let stdout_env_filter = EnvFilter::builder()
        .with_default_directive(service_config.log_level.into())
        .from_env_lossy();

    let stdout_layer = tracing_subscriber::fmt::layer()
        .compact()
        .with_writer(stdout_writer)
        .with_filter(stdout_env_filter);

    // File layer (if log_dir is set)
    if let Some(log_dir) = &service_config.log_dir {
        // Create the log directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(log_dir) {
            eprintln!(
                "Warning: Failed to create log directory {:?}: {}",
                log_dir, e
            );
        }

        let file_appender = tracing_appender::rolling::daily(log_dir, "jax.log");
        let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);
        guards.push(file_guard);

        let file_env_filter = EnvFilter::builder()
            .with_default_directive(service_config.log_level.into())
            .from_env_lossy();

        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(file_writer)
            .with_ansi(false)
            .with_span_events(FmtSpan::CLOSE)
            .with_filter(file_env_filter);

        tracing_subscriber::registry()
            .with(stdout_layer)
            .with(file_layer)
            .init();
    } else {
        tracing_subscriber::registry().with(stdout_layer).init();
    }

    utils::register_panic_logger();
    utils::report_build_info();

    guards
}

/// Create service state from config, exiting on error.
async fn create_state(service_config: &ServiceConfig) -> ServiceState {
    match ServiceState::from_config(service_config).await {
        Ok(state) => state,
        Err(e) => {
            tracing::error!("error creating server state: {}", e);
            std::process::exit(3);
        }
    }
}

/// Wait for shutdown and join all handles with timeout.
async fn shutdown_and_join(
    graceful_waiter: tokio::task::JoinHandle<()>,
    handles: Vec<tokio::task::JoinHandle<()>>,
) {
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

/// Spawns the daemon service based on config.
///
/// - Always spawns P2P peer for sync
/// - Spawns App server (UI + API) if app_port is set
/// - Spawns Gateway server if gateway_port is set
///
/// You can run:
/// - Just peer (sync only, no HTTP) - both ports None
/// - App only (UI + API) - app_port set, gateway_port None
/// - Gateway only - app_port None, gateway_port set
/// - App + Gateway - both ports set
pub async fn spawn_service(service_config: &ServiceConfig) {
    let _guards = init_logging(service_config);
    let (graceful_waiter, shutdown_rx) = utils::graceful_shutdown_blocker();
    let state = create_state(service_config).await;

    let mut handles = Vec::new();

    // Always spawn peer
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

    // Spawn App server if app_port is configured
    if let Some(app_port) = service_config.app_port {
        let app_listen_addr = SocketAddr::from_str(&format!("0.0.0.0:{}", app_port))
            .expect("Failed to parse app listen address");
        let app_state = state.as_ref().clone();
        let app_config = http_server::Config::new(
            app_listen_addr,
            service_config.api_url.clone(),
            service_config.gateway_url.clone(),
        );
        let app_rx = shutdown_rx.clone();
        let app_handle = tokio::spawn(async move {
            tracing::info!("Starting App server on {}", app_listen_addr);
            if let Err(e) = http_server::run_app(app_config, app_state, app_rx).await {
                tracing::error!("App server error: {}", e);
            }
        });
        handles.push(app_handle);
    }

    // Spawn Gateway server if gateway_port is configured
    if let Some(gateway_port) = service_config.gateway_port {
        let gateway_listen_addr = SocketAddr::from_str(&format!("0.0.0.0:{}", gateway_port))
            .expect("Failed to parse gateway listen address");
        let gateway_state = state.as_ref().clone();
        let gateway_config = http_server::Config::new(gateway_listen_addr, None, None);
        let gateway_rx = shutdown_rx.clone();
        let gateway_handle = tokio::spawn(async move {
            tracing::info!("Starting Gateway server on {}", gateway_listen_addr);
            if let Err(e) =
                http_server::run_gateway(gateway_config, gateway_state, gateway_rx).await
            {
                tracing::error!("Gateway server error: {}", e);
            }
        });
        handles.push(gateway_handle);
    }

    // Log what we're running
    match (service_config.app_port, service_config.gateway_port) {
        (Some(app), Some(gw)) => tracing::info!("Running: Peer + App:{} + Gateway:{}", app, gw),
        (Some(app), None) => tracing::info!("Running: Peer + App:{}", app),
        (None, Some(gw)) => tracing::info!("Running: Peer + Gateway:{}", gw),
        (None, None) => tracing::info!("Running: Peer only (no HTTP servers)"),
    }

    shutdown_and_join(graceful_waiter, handles).await;
}
