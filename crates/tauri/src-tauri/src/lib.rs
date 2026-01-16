//! JAX Bucket Tauri Desktop App
//!
//! This is a native desktop application for managing JAX buckets locally.
//! It runs a full P2P peer for syncing buckets with other nodes.
//!
//! **For serving buckets over HTTP (gateway), use the CLI: `jax daemon`**
//!
//! Supports `--config-path` argument to use custom config directories,
//! allowing it to run alongside CLI nodes for testing.

mod commands;

use std::path::PathBuf;
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};
use tokio::sync::watch;

use jax_bucket::{AppState as JaxAppState, ServiceConfig, ServiceState};

/// Global config path (set from CLI args before Tauri starts)
static CONFIG_PATH: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();

/// Application state shared across Tauri commands
pub struct AppState {
    /// The underlying JAX service state
    pub service: Arc<ServiceState>,
    /// Shutdown signal sender
    pub shutdown_tx: watch::Sender<()>,
    /// Config directory path for display
    pub config_dir: String,
}

/// Parse command line arguments before Tauri starts
fn parse_args() -> Option<PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    let mut config_path = None;

    let mut i = 1;
    while i < args.len() {
        if args[i] == "--config-path" {
            if i + 1 < args.len() {
                config_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            } else {
                i += 1;
            }
        } else if args[i].starts_with("--config-path=") {
            let path = args[i].strip_prefix("--config-path=").unwrap();
            config_path = Some(PathBuf::from(path));
            i += 1;
        } else {
            i += 1;
        }
    }

    config_path
}

/// Initialize and run the Tauri application
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Parse CLI args before Tauri starts
    let config_path = parse_args();
    CONFIG_PATH.set(config_path).ok();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            // Initialize tracing
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::from_default_env()
                        .add_directive(tracing::Level::INFO.into()),
                )
                .init();

            let config_path = CONFIG_PATH.get().cloned().flatten();
            let config_display = config_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "~/.jax".to_string());

            tracing::info!("Starting JAX Bucket Desktop App (config: {})", config_display);

            // Create shutdown channel
            let (shutdown_tx, shutdown_rx) = watch::channel(());

            // Initialize service in async context
            let handle = app.handle().clone();
            let config_path_clone = config_path.clone();
            let config_display_clone = config_display.clone();
            tauri::async_runtime::spawn(async move {
                // Load configuration using shared AppState
                let service_config = match load_config(config_path_clone) {
                    Ok(config) => config,
                    Err(e) => {
                        tracing::error!("Failed to load config: {}", e);
                        return;
                    }
                };

                // Create service state
                match ServiceState::from_config(&service_config).await {
                    Ok(service) => {
                        let service = Arc::new(service);

                        // Store state in Tauri
                        let app_state = AppState {
                            service: service.clone(),
                            shutdown_tx,
                            config_dir: config_display_clone.clone(),
                        };
                        handle.manage(app_state);

                        // Setup system tray
                        if let Err(e) = setup_tray(&handle, &config_display_clone) {
                            tracing::error!("Failed to setup tray: {}", e);
                        }

                        // Spawn P2P peer (full peer, not read-only)
                        let peer = service.peer().clone();
                        let peer_rx = shutdown_rx.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) = common::peer::spawn(peer, peer_rx).await {
                                tracing::error!("Peer error: {}", e);
                            }
                        });

                        tracing::info!("JAX Bucket desktop peer initialized");
                        tracing::info!("Peer ID: {}", service.peer().id());
                    }
                    Err(e) => {
                        tracing::error!("Failed to initialize service: {}", e);
                    }
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            // Hide window instead of closing when user clicks close button
            if let WindowEvent::CloseRequested { api, .. } = event {
                window.hide().unwrap_or_default();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_buckets,
            commands::create_bucket,
            commands::get_bucket,
            commands::list_files,
            commands::get_file,
            commands::add_file,
            commands::delete_file,
            commands::rename_file,
            commands::move_file,
            commands::create_directory,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Setup the system tray icon and menu
fn setup_tray<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    config_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create menu items
    let show_item = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
    let config_item = MenuItem::with_id(
        app,
        "config",
        format!("Config: {}", config_dir),
        false,
        None::<&str>,
    )?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    // Build the menu
    let menu = Menu::with_items(app, &[&show_item, &config_item, &quit_item])?;

    // Build and configure the tray icon
    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("JAX Bucket")
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "quit" => {
                    tracing::info!("Quitting JAX Bucket Desktop App");
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            // Show window on left click
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

/// Load configuration from AppState (shared with CLI)
fn load_config(config_path: Option<PathBuf>) -> Result<ServiceConfig, String> {
    // Load or initialize the app state using the shared module
    let app_state = JaxAppState::load_or_init(config_path.clone(), None)
        .map_err(|e| format!("Failed to load/init config: {}", e))?;

    // Convert to service config - desktop app has full access (not read-only)
    let mut service_config = app_state
        .to_service_config(false) // ui_read_only = false, full access
        .map_err(|e| format!("Failed to create service config: {}", e))?;

    // Desktop app doesn't run HTTP servers - that's what CLI daemon is for
    // IPC commands handle all bucket operations
    service_config.html_listen_addr = None;
    service_config.api_listen_addr = None;

    tracing::info!("Loaded config from: {:?}", app_state.jax_dir);
    if let Some(peer_port) = app_state.config.peer_port {
        tracing::info!("Peer port: {}", peer_port);
    }

    Ok(service_config)
}
