//! Blob store setup logic.

use std::path::Path;

use common::peer::BlobsStore;

use crate::state::BlobStoreConfig;

use super::BlobsSetupError;

/// Setup the blob store based on configuration.
///
/// Supports three modes:
/// - Legacy: Uses iroh's FsStore (default, for backwards compatibility)
/// - Filesystem: Uses SQLite + local filesystem via ObjectStore
/// - S3: Uses SQLite + S3/MinIO via ObjectStore
pub async fn setup_blobs_store(
    config: &BlobStoreConfig,
    jax_dir: &Path,
) -> Result<BlobsStore, BlobsSetupError> {
    match config {
        BlobStoreConfig::Legacy => {
            // Use iroh's FsStore for backwards compatibility
            let blobs_path = jax_dir.join("blobs");
            tracing::info!(path = %blobs_path.display(), "Using iroh blob store");
            BlobsStore::legacy_fs(&blobs_path)
                .await
                .map_err(|e| BlobsSetupError::StoreError(e.to_string()))
        }

        BlobStoreConfig::Filesystem { path } => {
            // Use ObjectStore with local filesystem backend
            tracing::info!(path = %path.display(), "Using SQLite + local filesystem blob store");
            BlobsStore::fs(path)
                .await
                .map_err(|e| BlobsSetupError::StoreError(e.to_string()))
        }

        BlobStoreConfig::S3 { url } => {
            // Parse S3 URL
            let s3_config = BlobStoreConfig::parse_s3_url(url)
                .map_err(|e| BlobsSetupError::StoreError(e.to_string()))?;

            tracing::info!(
                endpoint = %s3_config.endpoint,
                bucket = %s3_config.bucket,
                "Using SQLite + S3 blob store"
            );

            // SQLite database goes in jax_dir
            let db_path = jax_dir.join("blobs.db");

            BlobsStore::s3(
                &db_path,
                &s3_config.endpoint,
                &s3_config.access_key,
                &s3_config.secret_key,
                &s3_config.bucket,
                None, // Use default region
            )
            .await
            .map_err(|e| BlobsSetupError::StoreError(e.to_string()))
        }
    }
}
