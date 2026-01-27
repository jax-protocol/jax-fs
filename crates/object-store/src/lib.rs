//! SQLite + Object Storage Backend
//!
//! This crate provides an iroh-blobs compatible store implementation that uses
//! SQLite for metadata and pluggable object storage (S3/MinIO/local filesystem/memory)
//! for blob data.
//!
//! # Features
//!
//! - Content-addressed storage using BLAKE3 hashes (compatible with iroh-blobs)
//! - SQLite for fast metadata queries
//! - Multiple storage backends: S3, MinIO, local filesystem, in-memory
//! - Recovery support: rebuild metadata from object storage
//!
//! # Example
//!
//! ```rust,no_run
//! use jax_object_store::ObjectStore;
//! use std::path::Path;
//!
//! # async fn example() -> Result<(), jax_object_store::BlobStoreError> {
//! // Create a local file-based store
//! let store = ObjectStore::new_local(Path::new("/tmp/blobs")).await?;
//!
//! // Convert to iroh_blobs::api::Store for use with BlobsProtocol
//! let iroh_store: iroh_blobs::api::Store = store.into();
//! # Ok(())
//! # }
//! ```

mod actor;
mod database;
mod error;
mod object_store;
mod storage;

pub use error::{BlobStoreError, Result};
pub use object_store::ObjectStore;
pub use storage::ObjectStoreConfig;
