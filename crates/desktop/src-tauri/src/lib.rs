//! Jax Desktop Application - Tauri Backend
//!
//! This crate provides the Tauri backend for the Jax desktop application.
//! It embeds the jax daemon and exposes IPC commands that access
//! ServiceState directly (no HTTP proxying).

mod commands;
mod tray;

use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::RwLock;

use jax_daemon::ServiceState;

/// Inner daemon state, populated once the daemon has started.
pub struct DaemonInner {
    pub service: ServiceState,
    pub api_port: u16,
    pub gateway_port: u16,
    pub jax_dir: PathBuf,
}

/// Application state managed by Tauri.
/// Holds a direct reference to the daemon's ServiceState for IPC commands.
pub struct AppState {
    pub inner: Arc<RwLock<Option<DaemonInner>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(None)),
        }
    }
}

/// Run the Tauri application
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            // Initialize app state
            let state = AppState::default();
            app.manage(state);

            // Setup system tray
            tray::setup_tray(app)?;

            // Spawn daemon in background
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = spawn_daemon(&app_handle).await {
                    tracing::error!("Failed to spawn daemon: {}", e);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Bucket commands
            commands::bucket::list_buckets,
            commands::bucket::create_bucket,
            commands::bucket::delete_bucket,
            commands::bucket::ls,
            commands::bucket::cat,
            commands::bucket::add_file,
            commands::bucket::update_file,
            commands::bucket::rename_path,
            commands::bucket::move_path,
            commands::bucket::share_bucket,
            commands::bucket::is_published,
            commands::bucket::publish_bucket,
            commands::bucket::ping_peer,
            commands::bucket::upload_native_files,
            commands::bucket::mkdir,
            commands::bucket::delete_path,
            // History commands
            commands::bucket::get_history,
            commands::bucket::ls_at_version,
            commands::bucket::cat_at_version,
            // Share commands
            commands::bucket::get_bucket_shares,
            // Daemon commands
            commands::daemon::get_status,
            commands::daemon::get_identity,
            commands::daemon::get_config_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Spawn the jax daemon in the background
async fn spawn_daemon(app_handle: &tauri::AppHandle) -> Result<(), String> {
    use jax_daemon::state::AppState as JaxAppState;
    use jax_daemon::{start_service, ServiceConfig};

    // Load jax state from default location (~/.jax)
    let jax_state = JaxAppState::load(None)
        .map_err(|e| format!("Failed to load jax state (run 'jax init' first): {}", e))?;

    // Load the secret key
    let secret_key = jax_state
        .load_key()
        .map_err(|e| format!("Failed to load secret key: {}", e))?;

    // Build node listen address from peer_port if configured
    let node_listen_addr = jax_state.config.peer_port.map(|port| {
        format!("0.0.0.0:{}", port)
            .parse()
            .expect("Failed to parse peer listen address")
    });

    let api_port = jax_state.config.api_port;
    let gateway_port = jax_state.config.gateway_port;

    let state = app_handle.state::<AppState>();

    // Build service config
    let config = ServiceConfig {
        node_listen_addr,
        node_secret: Some(secret_key),
        blob_store: jax_state.config.blob_store.clone(),
        jax_dir: jax_state.jax_dir.clone(),
        api_port,
        gateway_port,
        sqlite_path: Some(jax_state.db_path),
        log_level: tracing::Level::INFO,
        log_dir: None,
        gateway_url: None,
    };

    tracing::info!(
        "Starting jax daemon: API on port {}, gateway on port {}",
        api_port,
        gateway_port
    );

    // Start the daemon and get direct state access
    let (service_state, shutdown_handle) = start_service(&config).await;

    // Store service state for IPC commands
    {
        let mut inner = state.inner.write().await;
        *inner = Some(DaemonInner {
            service: service_state,
            api_port,
            gateway_port,
            jax_dir: jax_state.jax_dir.clone(),
        });
    }

    // Block until shutdown
    shutdown_handle.wait().await;

    // Mark daemon as stopped
    {
        let mut inner = state.inner.write().await;
        *inner = None;
    }

    Ok(())
}
