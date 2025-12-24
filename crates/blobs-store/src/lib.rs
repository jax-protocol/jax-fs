// TODO: Remove this once the crate is integrated and all code is used
#![allow(dead_code)]

//! Object Store + SQLite blob store for iroh-blobs.
//!
//! This crate provides a blob store implementation that uses:
//! - **SQLite** for metadata (blob state, tags) - can be in-memory or persistent
//! - **Object storage** (S3/MinIO/GCS/Azure/local) for all blob data
//!
//! All blob data goes to object storage, enabling full recovery from storage alone.
//! SQLite serves as a fast metadata cache/index that can be rebuilt by scanning
//! object storage.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use jax_blobs_store::{BlobStore, ObjectStoreConfig};
//!
//! // Option 1: Local filesystem (no S3/MinIO needed)
//! let store = BlobStore::new_local("./data/blobs").await?;
//!
//! // Option 2: S3/MinIO backend
//! let config = ObjectStoreConfig::new(
//!     "http://localhost:9000",
//!     "minioadmin",
//!     "minioadmin",
//!     "jax-blobs",
//! );
//! let store = BlobStore::new("./blobs.db", config).await?;
//!
//! // Option 3: Fully in-memory (for testing)
//! let store = BlobStore::new_ephemeral().await?;
//!
//! // Store a blob
//! let hash = store.put(bytes::Bytes::from("hello world")).await?;
//!
//! // Retrieve a blob
//! let data = store.get(&hash).await?;
//! ```
//!
//! ## Recovery
//!
//! If the SQLite database is lost, you can rebuild it from object storage:
//!
//! ```rust,ignore
//! let store = BlobStore::in_memory(config).await?;
//! let stats = store.recover_from_storage().await?;
//! println!("Recovered {} blobs", stats.complete_blobs);
//! ```

mod bao_file;
mod database;
mod entry_state;
mod import;
mod object_store;
mod store;

pub use database::{Database, DatabaseError};
pub use object_store::{BlobObjectStore, ObjectStoreConfig, ObjectStoreError};
pub use store::{BlobStatus, BlobStore, StoreError};

/// Statistics from a recovery operation.
#[derive(Debug, Default)]
pub struct RecoveryStats {
    /// Number of complete blobs found
    pub complete_blobs: usize,
    /// Number of partial blobs found
    pub partial_blobs: usize,
    /// Number of orphaned objects cleaned up
    pub orphans_cleaned: usize,
    /// Number of tags recovered
    pub tags_recovered: usize,
}
