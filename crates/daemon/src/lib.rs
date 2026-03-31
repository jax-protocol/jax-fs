// Service modules (daemon functionality)
pub(crate) mod blobs;
pub mod clone_state;
pub(crate) mod database;
#[cfg(feature = "fuse")]
pub mod fuse;
pub mod http_server;
pub mod process;
pub mod service_config;
pub mod service_state;
pub(crate) mod sync_provider;
pub mod version;

// App state (configuration, paths)
pub mod state;

// Re-exports for consumers (Tauri, etc.)
pub use database::Database;
pub use process::{spawn_service, start_service, ShutdownHandle};
pub use service_config::Config as ServiceConfig;
pub use service_state::State as ServiceState;
pub use state::{AppConfig, AppState, BlobStoreConfig, StateError};

// Re-exports for mount management
pub use database::models::FuseMount;
pub use database::types::BucketAclEvent;
pub use database::types::MountStatus;
pub use version::build_info;
