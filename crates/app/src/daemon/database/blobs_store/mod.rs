//! Object Store + SQLite blob store implementation for iroh-blobs.
//!
//! This module provides a blob store that uses:
//! - SQLite for metadata (blob state, tags)
//! - Object storage (S3/MinIO/GCS/Azure/local) for all blob data and outboard storage
//!
//! All blob data goes to object storage, enabling full recovery from storage alone.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use jax_bucket::daemon::database::blobs_store::{BlobStoreApi, ObjectStoreConfig};
//!
//! // Create object store config for MinIO
//! let config = ObjectStoreConfig::new(
//!     "http://localhost:9000",
//!     "minioadmin",
//!     "minioadmin",
//!     "jax-blobs",
//! );
//!
//! // Create the blob store
//! let store = BlobStoreApi::new(database, BlobObjectStore::new_s3(config)?).await?;
//!
//! // Store a blob
//! let hash = store.put(bytes::Bytes::from("hello world")).await?;
//!
//! // Retrieve a blob
//! let data = store.get(&hash).await?;
//! ```

// TODO: Remove this once the module is integrated into the main application
#![allow(dead_code)]

#[allow(dead_code)]
mod actor;
#[allow(dead_code)]
mod bao_file;
#[allow(dead_code)]
mod entry_state;
#[allow(dead_code)]
mod import;
#[allow(dead_code)]
mod minio;

use thiserror::Error;

use crate::daemon::database::Database;
#[allow(unused_imports)]
pub use actor::BlobStatus;
pub use actor::BlobStoreApi;
pub use minio::{BlobObjectStore, ObjectStoreConfig, ObjectStoreError};

/// Configuration for the object store + SQLite blob store.
#[derive(Debug, Clone)]
pub struct StoreConfig {
    /// Object store configuration
    pub object_store: ObjectStoreConfig,
}

/// Errors that can occur when setting up or using the store.
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("object store error: {0}")]
    ObjectStore(#[from] ObjectStoreError),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Create a new object store + SQLite blob store.
///
/// # Arguments
/// * `db` - SQLite database connection pool
/// * `config` - Store configuration including object store settings
pub async fn create_store(db: Database, config: StoreConfig) -> Result<BlobStoreApi, StoreError> {
    // Create object store client
    let store = BlobObjectStore::new_s3(config.object_store)?;

    // Create the blob store API
    BlobStoreApi::new(db, store).await
}

/// Recover the SQLite metadata from object storage.
///
/// This scans all objects in storage and rebuilds the SQLite metadata tables.
/// Useful for disaster recovery or migrating to a new database.
pub async fn recover_from_storage(
    db: &Database,
    store: &BlobObjectStore,
) -> Result<RecoveryStats, StoreError> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let mut stats = RecoveryStats::default();

    // List all complete blob hashes
    let hashes = store.list_data_hashes().await?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    for hash_str in hashes {
        // Check if outboard exists
        let has_outboard = store.has_outboard(&hash_str).await;

        // Get size from object metadata
        let path = object_store::path::Path::from(format!("data/{}", hash_str));
        let size = match store.head(&path).await {
            Ok(info) => info.size as i64,
            Err(_) => continue,
        };

        // Insert into database
        let result = sqlx::query(
            r#"
            INSERT INTO blobs (hash, size, has_outboard, state, created_at, updated_at)
            VALUES (?, ?, ?, 'complete', ?, ?)
            ON CONFLICT(hash) DO NOTHING
            "#,
        )
        .bind(&hash_str)
        .bind(size)
        .bind(has_outboard)
        .bind(now)
        .bind(now)
        .execute(&**db)
        .await;

        if result.is_ok() {
            stats.complete_blobs += 1;
        }
    }

    Ok(stats)
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_config() {
        let config = StoreConfig {
            object_store: ObjectStoreConfig::new(
                "http://localhost:9000",
                "access",
                "secret",
                "jax-blobs",
            ),
        };
        assert_eq!(config.object_store.endpoint, "http://localhost:9000");
        assert_eq!(config.object_store.bucket, "jax-blobs");
    }
}
