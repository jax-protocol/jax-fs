//! Shared service infrastructure for JaxBucket gateway and local peer.
//!
//! This crate provides the common components used by both the gateway and local peer binaries:
//! - Database (SQLite with BucketLogProvider implementation)
//! - State management (ServiceState for peer + database)
//! - HTTP handlers (health checks, gateway, bucket explorer)
//! - Sync provider (background job queue for sync operations)

pub mod config;
pub mod database;
pub mod http;
pub mod sync_provider;
pub mod state;

// Re-export key types for convenience
pub use config::Config;
pub use database::{Database, DatabaseSetupError};
pub use state::{State as ServiceState, StateSetupError};
pub use sync_provider::{QueuedSyncConfig, QueuedSyncProvider, JobReceiver, run_worker};
