// Library exports for use by Tauri and other consumers

// Daemon/service modules (HTTP server, database, P2P sync)
pub mod daemon;

// Re-export key types for convenience
pub use daemon::http_server;
pub use daemon::AppConfig;
pub use daemon::AppState;
pub use daemon::AppStateError;
pub use daemon::PathHashMap;
pub use daemon::ServiceConfig;
pub use daemon::ServiceState;
pub use daemon::spawn_service;
