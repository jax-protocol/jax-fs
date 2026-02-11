//! System tray setup and handlers

use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, Manager,
};

use crate::AppState;

/// Holds a reference to the tray status menu item so we can update its text.
pub struct TrayState {
    pub status_item: MenuItem<tauri::Wry>,
}

/// Setup the system tray
pub fn setup_tray(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    // Create menu items
    let open = MenuItem::with_id(app, "open", "Open Jax", true, None::<&str>)?;
    let status = MenuItem::with_id(app, "status", "Status: Starting...", false, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    // Store the status item so we can update it later
    app.manage(TrayState {
        status_item: status.clone(),
    });

    // Build menu
    let menu = Menu::with_items(app, &[&open, &status, &quit])?;

    // Load tray icon from bundled resources
    let icon = Image::from_path("icons/tray-icon.png")?;

    // Build tray icon
    let _tray = TrayIconBuilder::with_id("main")
        .icon(icon)
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "open" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "status" => {
                // Status is informational, nothing to do
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    // Spawn task to update status periodically
    let app_handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            update_tray_status(&app_handle).await;
        }
    });

    Ok(())
}

/// Update the tray status menu item
async fn update_tray_status(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    let inner = state.inner.read().await;

    let status_text = match inner.as_ref() {
        Some(daemon) => format!(
            "Status: Running (API:{}, GW:{})",
            daemon.api_port, daemon.gateway_port
        ),
        None => "Status: Starting...".to_string(),
    };

    tracing::debug!("Tray status: {}", status_text);

    let tray_state = app.state::<TrayState>();
    let _ = tray_state.status_item.set_text(&status_text);
}
