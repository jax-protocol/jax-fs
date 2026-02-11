// Service modules (daemon functionality)
pub(crate) mod blobs;
pub mod clone_state;
pub(crate) mod database;
pub mod http_server;
pub mod process;
pub mod service_config;
pub mod service_state;
pub(crate) mod sync_provider;

// App state (configuration, paths)
pub mod state;

// Re-exports for consumers (Tauri, etc.)
pub use process::{spawn_service, start_service, ShutdownHandle};
pub use service_config::Config as ServiceConfig;
pub use service_state::State as ServiceState;
pub use state::{AppConfig, AppState, BlobStoreConfig, StateError};
