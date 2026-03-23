//! Jax Desktop Application - Tauri Backend
//!
//! This crate provides the Tauri backend for the Jax desktop application.
//! It connects to the jax daemon via HTTP API, either detecting an
//! already-running sidecar daemon or spawning an embedded one.

mod commands;
mod tray;
pub mod version;

use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::RwLock;

use common::version::BuildInfo;
use jax_daemon::http_server::api::client::ApiClient;
use jax_daemon::http_server::health::liveness::LivezRequest;
use jax_daemon::http_server::health::version::VersionRequest;
use reqwest::Url;

/// How the desktop app is connected to the daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonMode {
    /// Desktop spawned the daemon in-process.
    Embedded,
    /// Desktop connected to an already-running daemon.
    Sidecar,
}

impl fmt::Display for DaemonMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DaemonMode::Embedded => write!(f, "embedded"),
            DaemonMode::Sidecar => write!(f, "sidecar"),
        }
    }
}

/// Inner daemon state, populated once the daemon is available.
/// No longer holds a direct `ServiceState` reference — all access goes through HTTP.
pub struct DaemonInner {
    pub client: ApiClient,
    pub api_port: u16,
    pub gateway_port: u16,
    pub jax_dir: PathBuf,
    pub log_dir: PathBuf,
    pub mode: DaemonMode,
    /// Build info from the daemon, used to check feature availability.
    pub build_info: Option<BuildInfo>,
}

/// Application state managed by Tauri.
pub struct AppState {
    pub inner: Arc<RwLock<Option<DaemonInner>>>,
    /// Whether log streaming is active (controls the background tail task).
    pub log_streaming: Arc<AtomicBool>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(None)),
            log_streaming: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// Run the Tauri application
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Tracing is initialized later in connect_or_spawn_daemon once we know jax_dir,
    // so that logs can also go to a file. Early startup messages use eprintln.

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                use tauri::ActivationPolicy;
                app.set_activation_policy(ActivationPolicy::Accessory);
            }
            let state = AppState::default();
            app.manage(state);

            tray::setup_tray(app)?;

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = connect_or_spawn_daemon(&app_handle).await {
                    tracing::error!("Failed to connect to daemon: {}", e);
                    eprintln!("DAEMON ERROR: {}", e);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Bucket commands
            commands::bucket::list_buckets,
            commands::bucket::create_bucket,
            commands::bucket::delete_bucket,
            commands::bucket::remove_bucket,
            commands::bucket::ls,
            commands::bucket::cat,
            commands::bucket::add_file,
            commands::bucket::update_file,
            commands::bucket::rename_path,
            commands::bucket::move_path,
            commands::bucket::share_bucket,
            commands::bucket::is_published,
            commands::bucket::publish_bucket,
            commands::bucket::unpublish_bucket,
            commands::bucket::ping_peer,
            commands::bucket::upload_native_files,
            commands::bucket::export_file,
            commands::bucket::mkdir,
            commands::bucket::delete_path,
            // History commands
            commands::bucket::get_history,
            commands::bucket::ls_at_version,
            commands::bucket::cat_at_version,
            // Share commands
            commands::bucket::get_bucket_shares,
            commands::bucket::remove_share,
            // Daemon commands
            commands::daemon::get_status,
            commands::daemon::get_identity,
            commands::daemon::get_config_info,
            // Mount commands
            commands::mount::list_mounts,
            commands::mount::create_mount,
            commands::mount::get_mount,
            commands::mount::update_mount,
            commands::mount::delete_mount,
            commands::mount::start_mount,
            commands::mount::stop_mount,
            commands::mount::is_fuse_available,
            // Simplified mount API for desktop
            commands::mount::mount_bucket,
            commands::mount::unmount_bucket,
            commands::mount::is_bucket_mounted,
            // Log commands
            commands::logs::list_log_files,
            commands::logs::read_log_file,
            commands::logs::tail_log_file,
            commands::logs::subscribe_logs,
            commands::logs::unsubscribe_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Probe whether a daemon is already listening on the given port.
/// Returns `true` if a healthy daemon was detected.
async fn probe_daemon(api_port: u16) -> bool {
    let url = format!("http://localhost:{}", api_port);
    let base_url = match Url::parse(&url) {
        Ok(u) => u,
        Err(_) => return false,
    };
    let mut client = match ApiClient::new(&base_url) {
        Ok(c) => c,
        Err(_) => return false,
    };

    client.call(LivezRequest {}).await.is_ok()
}

/// Initialize tracing with stdout and file layers.
/// Must be called before any tracing macros are used.
fn init_desktop_logging(log_dir: &std::path::Path) {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::{EnvFilter, Layer};

    let _ = std::fs::create_dir_all(log_dir);

    let env_filter = || {
        EnvFilter::builder()
            .with_default_directive(tracing::level_filters::LevelFilter::WARN.into())
            .from_env_lossy()
            .add_directive("jax_daemon=info".parse().unwrap())
            .add_directive("jax_common=info".parse().unwrap())
            .add_directive("jax_desktop=info".parse().unwrap())
    };

    let stdout_layer = tracing_subscriber::fmt::layer()
        .compact()
        .with_filter(env_filter());

    let file_appender = tracing_appender::rolling::daily(log_dir, "jax-desktop.log");
    let (file_writer, _guard) = tracing_appender::non_blocking(file_appender);
    // Leak the guard so the writer lives for the process lifetime
    std::mem::forget(_guard);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_writer)
        .with_ansi(false)
        .with_filter(env_filter());

    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(file_layer)
        .init();
}

/// Try to connect to an existing sidecar daemon; if none found, spawn an embedded one.
async fn connect_or_spawn_daemon(app_handle: &tauri::AppHandle) -> Result<(), String> {
    use jax_daemon::state::AppState as JaxAppState;

    // Load jax state to get configured ports and paths
    let jax_state = JaxAppState::load(None)
        .map_err(|e| format!("Failed to load jax state (run 'jax init' first): {}", e))?;

    let api_port = jax_state.config.api_port;
    let gateway_port = jax_state.config.gateway_port;
    let jax_dir = jax_state.jax_dir.clone();
    let log_dir = jax_dir.join("logs");

    // Initialize tracing now that we know jax_dir
    init_desktop_logging(&log_dir);

    let state = app_handle.state::<AppState>();

    // Phase 1: probe for an already-running daemon
    if probe_daemon(api_port).await {
        tracing::info!(
            "Detected running sidecar daemon on port {}, using sidecar mode",
            api_port
        );

        // Emit connection-mode event to the frontend
        let _ = app_handle.emit("daemon-mode", "sidecar");

        let base_url = Url::parse(&format!("http://localhost:{}", api_port))
            .map_err(|e| format!("Failed to parse URL: {}", e))?;
        let client =
            ApiClient::new(&base_url).map_err(|e| format!("Failed to create API client: {}", e))?;

        // Fetch daemon build info to check feature availability
        let build_info = {
            let mut c = client.clone();
            c.call(VersionRequest {}).await.ok()
        };

        let mut inner = state.inner.write().await;
        *inner = Some(DaemonInner {
            client,
            api_port,
            gateway_port,
            jax_dir,
            log_dir,
            mode: DaemonMode::Sidecar,
            build_info,
        });

        return Ok(());
    }

    // Phase 2: no sidecar detected — start an embedded daemon
    tracing::info!(
        "No sidecar daemon detected, starting embedded daemon on ports {}/{}",
        api_port,
        gateway_port
    );

    let secret_key = jax_state
        .load_key()
        .map_err(|e| format!("Failed to load secret key: {}", e))?;

    let node_listen_addr = jax_state.config.peer_port.map(|port| {
        format!("0.0.0.0:{}", port)
            .parse()
            .expect("Failed to parse peer listen address")
    });

    let config = jax_daemon::ServiceConfig {
        node_listen_addr,
        node_secret: Some(secret_key),
        blob_store: jax_state.config.blob_store.clone(),
        jax_dir: jax_state.jax_dir.clone(),
        max_import_size: jax_state.config.max_import_size,
        api_port,
        gateway_port,
        sqlite_path: Some(jax_state.db_path),
        log_level: tracing::Level::INFO,
        log_dir: Some(log_dir.clone()),
        gateway_url: None,
    };

    let (_service_state, shutdown_handle) = jax_daemon::start_service(&config).await;

    // Emit connection-mode event to the frontend
    let _ = app_handle.emit("daemon-mode", "embedded");

    let base_url = Url::parse(&format!("http://localhost:{}", api_port))
        .map_err(|e| format!("Failed to parse URL: {}", e))?;
    let client =
        ApiClient::new(&base_url).map_err(|e| format!("Failed to create API client: {}", e))?;

    // Fetch daemon build info to check feature availability
    let build_info = {
        let mut c = client.clone();
        c.call(VersionRequest {}).await.ok()
    };

    {
        let mut inner = state.inner.write().await;
        *inner = Some(DaemonInner {
            client,
            api_port,
            gateway_port,
            jax_dir: jax_state.jax_dir.clone(),
            log_dir,
            mode: DaemonMode::Embedded,
            build_info,
        });
    }

    // Block until shutdown
    shutdown_handle.wait().await;

    {
        let mut inner = state.inner.write().await;
        *inner = None;
    }

    Ok(())
}
