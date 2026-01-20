//! Low-level blob store setup logic for each backend type.

use std::path::Path;

use common::peer::BlobsStore;

use crate::state::{BlobStoreConfig, S3Config};

use super::BlobsSetupError;

/// Setup the blob store based on configuration.
/// Returns the legacy BlobsStore for the Peer.
pub async fn setup_blobs_store(
    config: &BlobStoreConfig,
    jax_dir: &Path,
) -> Result<BlobsStore, BlobsSetupError> {
    match config {
        BlobStoreConfig::Legacy => {
            // Legacy mode: use the jax_dir/blobs path
            let blobs_path = jax_dir.join("blobs");

            tracing::info!(path = %blobs_path.display(), "Using legacy iroh blob store");
            let blobs = BlobsStore::fs(&blobs_path)
                .await
                .map_err(|e| BlobsSetupError::StoreError(e.to_string()))?;

            Ok(blobs)
        }

        BlobStoreConfig::Filesystem { path } => {
            // Filesystem mode: use SQLite + local object storage
            // Path is absolute (set at init time)
            tracing::info!(path = %path.display(), "Using filesystem blob store (SQLite + local objects)");

            // Create the new blob store for actual storage
            let _new_store = blobs_store::BlobStore::new_local(path)
                .await
                .map_err(|e| BlobsSetupError::StoreError(e.to_string()))?;

            // For now, we still need a legacy BlobsStore for the Peer
            // Use a subdirectory for legacy compatibility
            let legacy_path = path.join("legacy-blobs");
            let blobs = BlobsStore::fs(&legacy_path)
                .await
                .map_err(|e| BlobsSetupError::StoreError(e.to_string()))?;

            Ok(blobs)
        }

        BlobStoreConfig::S3 { url } => {
            // Parse S3 URL into components
            let s3_config = BlobStoreConfig::parse_s3_url(url)
                .map_err(|e| BlobsSetupError::StoreError(e.to_string()))?;

            tracing::info!(
                endpoint = %s3_config.endpoint,
                bucket = %s3_config.bucket,
                "Using S3 blob store"
            );

            // Determine SQLite path for metadata
            let db_path = jax_dir.join("blobs-store.db");

            // Create the S3 object store config
            let object_config = to_object_store_config(&s3_config);

            // Create the new blob store
            let _new_store = blobs_store::BlobStore::new(&db_path, object_config)
                .await
                .map_err(|e| BlobsSetupError::StoreError(e.to_string()))?;

            // For now, we still need a legacy BlobsStore for the Peer
            // Use a cache directory for S3 mode
            let cache_path = jax_dir.join("blobs-cache");
            let blobs = BlobsStore::fs(&cache_path)
                .await
                .map_err(|e| BlobsSetupError::StoreError(e.to_string()))?;

            Ok(blobs)
        }
    }
}

/// Convert parsed S3Config to blobs_store::ObjectStoreConfig
fn to_object_store_config(config: &S3Config) -> blobs_store::ObjectStoreConfig {
    blobs_store::ObjectStoreConfig::S3 {
        endpoint: config.endpoint.clone(),
        access_key: config.access_key.clone(),
        secret_key: config.secret_key.clone(),
        bucket: config.bucket.clone(),
        region: None, // Region not needed for MinIO
    }
}
