//! FUSE filesystem integration for jax-bucket
//!
//! This module provides FUSE-based filesystem mounting for buckets, allowing
//! users to interact with bucket contents as local files.
//!
//! # Architecture
//!
//! - `MountManager`: Manages live mounts and their lifecycle
//! - `JaxFs`: FUSE filesystem implementation using fuser
//! - `InodeTable`: Bidirectional inode ↔ path mapping
//! - `FileCache`: LRU cache with TTL for file contents
//!
//! # Sync Integration
//!
//! The FUSE filesystem subscribes to sync events from the daemon's sync engine.
//! When remote changes arrive (via peer sync), the cache is invalidated and
//! FUSE operations see the updated content.

mod cache;
mod inode_table;
mod jax_fs;
mod mount_manager;
mod sync_events;

pub use cache::{CacheStats, FileCache, FileCacheConfig};
pub use inode_table::InodeTable;
pub use jax_fs::JaxFs;
pub use mount_manager::{LiveMount, MountError, MountManager, MountManagerConfig};
pub use sync_events::SyncEvent;
