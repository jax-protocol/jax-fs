//! Object store client wrapper for blob storage.
//!
//! Uses the `object_store` crate for a unified interface to S3/MinIO/GCS/Azure/local storage.
//! The same code works across all backends with just configuration changes.

use std::sync::Arc;

use bytes::Bytes;
use futures::TryStreamExt;
use object_store::aws::AmazonS3Builder;
use object_store::path::Path;
use object_store::{ObjectStore, PutPayload};
use thiserror::Error;
use tracing::{debug, info};

/// Default path prefixes for organizing blobs
const DATA_PREFIX: &str = "data";
const OUTBOARD_PREFIX: &str = "outboard";
const PARTIAL_PREFIX: &str = "partial";

/// Configuration for connecting to S3/MinIO object storage.
#[derive(Debug, Clone)]
pub struct ObjectStoreConfig {
    /// Endpoint URL (e.g., "http://localhost:9000" for MinIO)
    pub endpoint: String,
    /// Access key ID
    pub access_key: String,
    /// Secret access key
    pub secret_key: String,
    /// Bucket name
    pub bucket: String,
    /// Region (defaults to "us-east-1" for MinIO compatibility)
    pub region: Option<String>,
    /// Whether to allow HTTP (non-HTTPS) connections
    pub allow_http: bool,
}

impl ObjectStoreConfig {
    /// Create a new config for MinIO/S3.
    pub fn new(
        endpoint: impl Into<String>,
        access_key: impl Into<String>,
        secret_key: impl Into<String>,
        bucket: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            access_key: access_key.into(),
            secret_key: secret_key.into(),
            bucket: bucket.into(),
            region: None,
            allow_http: true, // MinIO typically uses HTTP locally
        }
    }

    /// Set the region.
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    /// Set whether to allow HTTP connections.
    pub fn with_allow_http(mut self, allow: bool) -> Self {
        self.allow_http = allow;
        self
    }
}

/// Errors from object store operations.
#[derive(Debug, Error)]
pub enum ObjectStoreError {
    #[error("object store error: {0}")]
    Store(#[from] object_store::Error),

    #[error("object not found: {0}")]
    NotFound(String),

    #[error("configuration error: {0}")]
    Config(String),
}

/// Client for interacting with object storage (S3/MinIO/etc).
///
/// Uses the `object_store` crate which provides a unified interface
/// for AWS S3, MinIO, GCS, Azure Blob Storage, and local files.
#[derive(Debug, Clone)]
pub struct BlobObjectStore {
    store: Arc<dyn ObjectStore>,
    bucket: String,
}

impl BlobObjectStore {
    /// Create a new object store client from S3/MinIO configuration.
    pub fn new_s3(config: ObjectStoreConfig) -> Result<Self, ObjectStoreError> {
        let mut builder = AmazonS3Builder::new()
            .with_endpoint(&config.endpoint)
            .with_access_key_id(&config.access_key)
            .with_secret_access_key(&config.secret_key)
            .with_bucket_name(&config.bucket)
            .with_region(config.region.as_deref().unwrap_or("us-east-1"));

        if config.allow_http {
            builder = builder.with_allow_http(true);
        }

        let store = builder
            .build()
            .map_err(|e| ObjectStoreError::Config(e.to_string()))?;

        Ok(Self {
            store: Arc::new(store),
            bucket: config.bucket,
        })
    }

    /// Create from an existing ObjectStore implementation.
    /// Useful for testing with in-memory stores or other backends.
    pub fn new_from_store(store: Arc<dyn ObjectStore>, bucket: String) -> Self {
        Self { store, bucket }
    }

    /// Get the bucket name.
    pub fn bucket(&self) -> &str {
        &self.bucket
    }

    /// Get the underlying object store.
    pub fn inner(&self) -> &Arc<dyn ObjectStore> {
        &self.store
    }

    // Path helpers

    fn data_path(hash: &str) -> Path {
        Path::from(format!("{}/{}", DATA_PREFIX, hash))
    }

    fn outboard_path(hash: &str) -> Path {
        Path::from(format!("{}/{}", OUTBOARD_PREFIX, hash))
    }

    fn partial_data_path(hash: &str) -> Path {
        Path::from(format!("{}/{}/data", PARTIAL_PREFIX, hash))
    }

    fn partial_outboard_path(hash: &str) -> Path {
        Path::from(format!("{}/{}/outboard", PARTIAL_PREFIX, hash))
    }

    fn partial_bitfield_path(hash: &str) -> Path {
        Path::from(format!("{}/{}/bitfield", PARTIAL_PREFIX, hash))
    }

    // Core operations

    /// Put an object at the given path.
    /// This is guaranteed to be atomic.
    pub async fn put(&self, path: &Path, data: Bytes) -> Result<(), ObjectStoreError> {
        self.store.put(path, PutPayload::from(data)).await?;
        debug!("put object: {}", path);
        Ok(())
    }

    /// Get an object from the given path.
    pub async fn get(&self, path: &Path) -> Result<Bytes, ObjectStoreError> {
        let result = self.store.get(path).await?;
        let bytes = result.bytes().await?;
        debug!("get object: {} ({} bytes)", path, bytes.len());
        Ok(bytes)
    }

    /// Get a range of bytes from an object.
    pub async fn get_range(
        &self,
        path: &Path,
        offset: usize,
        length: usize,
    ) -> Result<Bytes, ObjectStoreError> {
        let range = offset..offset + length;
        let bytes = self.store.get_range(path, range).await?;
        debug!("get range: {} offset={} len={}", path, offset, bytes.len());
        Ok(bytes)
    }

    /// Check if an object exists and get its metadata.
    pub async fn head(&self, path: &Path) -> Result<ObjectInfo, ObjectStoreError> {
        let meta = self.store.head(path).await?;
        Ok(ObjectInfo {
            path: meta.location.to_string(),
            size: meta.size as u64,
            last_modified: meta.last_modified,
        })
    }

    /// Check if an object exists.
    pub async fn exists(&self, path: &Path) -> bool {
        self.store.head(path).await.is_ok()
    }

    /// Delete an object.
    pub async fn delete(&self, path: &Path) -> Result<(), ObjectStoreError> {
        self.store.delete(path).await?;
        debug!("delete object: {}", path);
        Ok(())
    }

    /// List objects with a given prefix.
    pub async fn list(&self, prefix: Option<&Path>) -> Result<Vec<ObjectInfo>, ObjectStoreError> {
        let stream = self.store.list(prefix);
        let objects: Vec<_> = stream
            .map_ok(|meta| ObjectInfo {
                path: meta.location.to_string(),
                size: meta.size as u64,
                last_modified: meta.last_modified,
            })
            .try_collect()
            .await?;

        debug!("list objects: prefix={:?} count={}", prefix, objects.len());
        Ok(objects)
    }

    // Convenience methods for blob storage

    /// Store complete blob data.
    pub async fn put_data(&self, hash: &str, data: Bytes) -> Result<(), ObjectStoreError> {
        let path = Self::data_path(hash);
        self.put(&path, data).await?;
        info!("stored blob data: {}", hash);
        Ok(())
    }

    /// Get complete blob data.
    pub async fn get_data(&self, hash: &str) -> Result<Bytes, ObjectStoreError> {
        let path = Self::data_path(hash);
        self.get(&path).await
    }

    /// Get a range of blob data (for BAO streaming).
    pub async fn get_data_range(
        &self,
        hash: &str,
        offset: usize,
        length: usize,
    ) -> Result<Bytes, ObjectStoreError> {
        let path = Self::data_path(hash);
        self.get_range(&path, offset, length).await
    }

    /// Check if blob data exists.
    pub async fn has_data(&self, hash: &str) -> bool {
        let path = Self::data_path(hash);
        self.exists(&path).await
    }

    /// Delete blob data.
    pub async fn delete_data(&self, hash: &str) -> Result<(), ObjectStoreError> {
        let path = Self::data_path(hash);
        self.delete(&path).await
    }

    /// Store outboard data.
    pub async fn put_outboard(&self, hash: &str, data: Bytes) -> Result<(), ObjectStoreError> {
        let path = Self::outboard_path(hash);
        self.put(&path, data).await
    }

    /// Get outboard data.
    pub async fn get_outboard(&self, hash: &str) -> Result<Bytes, ObjectStoreError> {
        let path = Self::outboard_path(hash);
        self.get(&path).await
    }

    /// Get a range of outboard data (for BAO streaming).
    pub async fn get_outboard_range(
        &self,
        hash: &str,
        offset: usize,
        length: usize,
    ) -> Result<Bytes, ObjectStoreError> {
        let path = Self::outboard_path(hash);
        self.get_range(&path, offset, length).await
    }

    /// Check if outboard data exists.
    pub async fn has_outboard(&self, hash: &str) -> bool {
        let path = Self::outboard_path(hash);
        self.exists(&path).await
    }

    /// Delete outboard data.
    pub async fn delete_outboard(&self, hash: &str) -> Result<(), ObjectStoreError> {
        let path = Self::outboard_path(hash);
        self.delete(&path).await
    }

    /// Store partial blob data.
    pub async fn put_partial_data(&self, hash: &str, data: Bytes) -> Result<(), ObjectStoreError> {
        let path = Self::partial_data_path(hash);
        self.put(&path, data).await
    }

    /// Store partial outboard data.
    pub async fn put_partial_outboard(
        &self,
        hash: &str,
        data: Bytes,
    ) -> Result<(), ObjectStoreError> {
        let path = Self::partial_outboard_path(hash);
        self.put(&path, data).await
    }

    /// Store partial bitfield.
    pub async fn put_partial_bitfield(
        &self,
        hash: &str,
        bitfield: Bytes,
    ) -> Result<(), ObjectStoreError> {
        let path = Self::partial_bitfield_path(hash);
        self.put(&path, bitfield).await
    }

    /// Delete all partial data for a hash.
    pub async fn delete_partial(&self, hash: &str) -> Result<(), ObjectStoreError> {
        let prefix = Path::from(format!("{}/{}/", PARTIAL_PREFIX, hash));
        let objects = self.list(Some(&prefix)).await?;

        for obj in objects {
            let path = Path::from(obj.path);
            let _ = self.delete(&path).await;
        }

        Ok(())
    }

    /// Promote partial to complete (move from partial to data/outboard).
    pub async fn promote_partial(
        &self,
        hash: &str,
        data: Bytes,
        outboard: Option<Bytes>,
    ) -> Result<(), ObjectStoreError> {
        // Store in final locations
        self.put_data(hash, data).await?;

        if let Some(ob) = outboard {
            self.put_outboard(hash, ob).await?;
        }

        // Clean up partial
        self.delete_partial(hash).await?;

        info!("promoted partial to complete: {}", hash);
        Ok(())
    }

    /// List all complete blob hashes.
    pub async fn list_data_hashes(&self) -> Result<Vec<String>, ObjectStoreError> {
        let prefix = Path::from(DATA_PREFIX);
        let objects = self.list(Some(&prefix)).await?;

        let hashes: Vec<String> = objects
            .into_iter()
            .filter_map(|obj| {
                // Extract hash from path like "data/abc123"
                obj.path
                    .strip_prefix(&format!("{}/", DATA_PREFIX))
                    .map(|s| s.to_string())
            })
            .collect();

        Ok(hashes)
    }
}

/// Information about an object in storage.
#[derive(Debug, Clone)]
pub struct ObjectInfo {
    /// Object path
    pub path: String,
    /// Object size in bytes
    pub size: u64,
    /// Last modified time
    pub last_modified: chrono::DateTime<chrono::Utc>,
}

// Re-export for convenience
#[allow(unused_imports)]
pub use object_store::path::Path as ObjectPath;

#[cfg(test)]
mod tests {
    use super::*;
    use object_store::memory::InMemory;

    #[tokio::test]
    async fn test_in_memory_store() {
        let store = Arc::new(InMemory::new());
        let blob_store = BlobObjectStore::new_from_store(store, "test-bucket".to_string());

        let hash = "abc123";
        let data = Bytes::from("hello world");

        // Put
        blob_store.put_data(hash, data.clone()).await.unwrap();

        // Has
        assert!(blob_store.has_data(hash).await);

        // Get
        let retrieved = blob_store.get_data(hash).await.unwrap();
        assert_eq!(data, retrieved);

        // Get range
        let range = blob_store.get_data_range(hash, 0, 5).await.unwrap();
        assert_eq!(range, Bytes::from("hello"));

        // List
        let hashes = blob_store.list_data_hashes().await.unwrap();
        assert_eq!(hashes, vec!["abc123"]);

        // Delete
        blob_store.delete_data(hash).await.unwrap();
        assert!(!blob_store.has_data(hash).await);
    }

    #[tokio::test]
    async fn test_outboard_operations() {
        let store = Arc::new(InMemory::new());
        let blob_store = BlobObjectStore::new_from_store(store, "test-bucket".to_string());

        let hash = "def456";
        let outboard = Bytes::from(vec![0u8; 64]);

        blob_store
            .put_outboard(hash, outboard.clone())
            .await
            .unwrap();
        assert!(blob_store.has_outboard(hash).await);

        let retrieved = blob_store.get_outboard(hash).await.unwrap();
        assert_eq!(outboard, retrieved);
    }

    #[tokio::test]
    async fn test_partial_operations() {
        let store = Arc::new(InMemory::new());
        let blob_store = BlobObjectStore::new_from_store(store, "test-bucket".to_string());

        let hash = "partial123";
        let data = Bytes::from("partial data");
        let outboard = Bytes::from("partial outboard");

        // Store partial
        blob_store
            .put_partial_data(hash, data.clone())
            .await
            .unwrap();
        blob_store
            .put_partial_outboard(hash, outboard.clone())
            .await
            .unwrap();

        // Promote to complete
        blob_store
            .promote_partial(hash, data.clone(), Some(outboard))
            .await
            .unwrap();

        // Should now exist as complete
        assert!(blob_store.has_data(hash).await);
        assert!(blob_store.has_outboard(hash).await);
    }
}
