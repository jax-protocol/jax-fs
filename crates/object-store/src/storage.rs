//! Object storage backend abstraction (S3/MinIO/local filesystem/memory).

use std::path::PathBuf;
use std::sync::Arc;

use bytes::Bytes;
use object_store::aws::AmazonS3Builder;
use object_store::local::LocalFileSystem;
use object_store::memory::InMemory;
use object_store::path::Path as ObjectPath;
use object_store::ObjectStore;
use serde::{Deserialize, Serialize};

use crate::error::{BlobStoreError, Result};

/// Configuration for the object storage backend.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ObjectStoreConfig {
    /// In-memory storage (for testing)
    #[default]
    Memory,

    /// Local filesystem storage
    Local {
        /// Path to the storage directory
        path: PathBuf,
    },

    /// S3-compatible storage (AWS S3, MinIO, etc.)
    S3 {
        /// S3 endpoint URL (e.g., "http://localhost:9000" for MinIO)
        endpoint: String,
        /// Access key ID
        access_key: String,
        /// Secret access key
        secret_key: String,
        /// Bucket name
        bucket: String,
        /// Optional region (defaults to "us-east-1")
        region: Option<String>,
    },
}

/// Wrapper around different object storage backends.
#[derive(Debug, Clone)]
pub(crate) struct Storage {
    inner: Arc<dyn ObjectStore>,
}

impl Storage {
    /// Create a new storage backend from configuration.
    pub async fn new(config: ObjectStoreConfig) -> Result<Self> {
        let inner: Arc<dyn ObjectStore> = match &config {
            ObjectStoreConfig::Memory => Arc::new(InMemory::new()),

            ObjectStoreConfig::Local { path } => {
                // Ensure directory exists
                tokio::fs::create_dir_all(path).await?;
                Arc::new(
                    LocalFileSystem::new_with_prefix(path)
                        .map_err(|e| BlobStoreError::InvalidConfig(e.to_string()))?,
                )
            }

            ObjectStoreConfig::S3 {
                endpoint,
                access_key,
                secret_key,
                bucket,
                region,
            } => {
                let builder = AmazonS3Builder::new()
                    .with_endpoint(endpoint)
                    .with_access_key_id(access_key)
                    .with_secret_access_key(secret_key)
                    .with_bucket_name(bucket)
                    .with_region(region.as_deref().unwrap_or("us-east-1"))
                    .with_allow_http(endpoint.starts_with("http://"));

                let store: Arc<dyn ObjectStore> = Arc::new(
                    builder
                        .build()
                        .map_err(|e| BlobStoreError::InvalidConfig(e.to_string()))?,
                );

                // Verify bucket exists by listing (empty prefix)
                // This will fail fast if the bucket doesn't exist
                {
                    use futures::TryStreamExt;
                    let prefix = ObjectPath::from("");
                    let mut stream = store.list(Some(&prefix));
                    match stream.try_next().await {
                        Ok(_) => {} // Bucket exists (may or may not have items)
                        Err(object_store::Error::NotFound { .. }) => {
                            return Err(BlobStoreError::BucketNotFound(bucket.clone()));
                        }
                        Err(e) => {
                            // Check if error message indicates bucket doesn't exist
                            let msg = e.to_string();
                            if msg.contains("NoSuchBucket")
                                || msg.contains("bucket") && msg.contains("not")
                            {
                                return Err(BlobStoreError::BucketNotFound(bucket.clone()));
                            }
                            return Err(e.into());
                        }
                    }
                }

                store
            }
        };

        Ok(Self { inner })
    }

    /// Build the object path for blob data.
    fn data_path(hash: &str) -> ObjectPath {
        ObjectPath::from(format!("data/{}", hash))
    }

    /// Build the object path for blob outboard data.
    fn outboard_path(hash: &str) -> ObjectPath {
        ObjectPath::from(format!("outboard/{}", hash))
    }

    /// Put blob data into storage.
    pub async fn put_data(&self, hash: &str, data: Bytes) -> Result<()> {
        let path = Self::data_path(hash);
        self.inner.put(&path, data.into()).await?;
        Ok(())
    }

    /// Get blob data from storage.
    pub async fn get_data(&self, hash: &str) -> Result<Option<Bytes>> {
        let path = Self::data_path(hash);
        match self.inner.get(&path).await {
            Ok(result) => {
                let bytes = result.bytes().await?;
                Ok(Some(bytes))
            }
            Err(object_store::Error::NotFound { .. }) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Delete blob data from storage.
    pub async fn delete_data(&self, hash: &str) -> Result<()> {
        let path = Self::data_path(hash);
        // Ignore NotFound errors - the blob may already be deleted
        match self.inner.delete(&path).await {
            Ok(()) => Ok(()),
            Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Delete blob outboard data from storage.
    pub async fn delete_outboard(&self, hash: &str) -> Result<()> {
        let path = Self::outboard_path(hash);
        // Ignore NotFound errors
        match self.inner.delete(&path).await {
            Ok(()) => Ok(()),
            Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
impl Storage {
    /// Create an in-memory storage backend (test-only).
    pub fn memory() -> Self {
        Self {
            inner: Arc::new(InMemory::new()),
        }
    }

    /// Put blob outboard data into storage.
    pub async fn put_outboard(&self, hash: &str, data: Bytes) -> Result<()> {
        let path = Self::outboard_path(hash);
        self.inner.put(&path, data.into()).await?;
        Ok(())
    }

    /// Get blob outboard data from storage.
    pub async fn get_outboard(&self, hash: &str) -> Result<Option<Bytes>> {
        let path = Self::outboard_path(hash);
        match self.inner.get(&path).await {
            Ok(result) => {
                let bytes = result.bytes().await?;
                Ok(Some(bytes))
            }
            Err(object_store::Error::NotFound { .. }) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Check if blob data exists in storage.
    pub async fn has_data(&self, hash: &str) -> Result<bool> {
        let path = Self::data_path(hash);
        match self.inner.head(&path).await {
            Ok(_) => Ok(true),
            Err(object_store::Error::NotFound { .. }) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// List all blob hashes in the data directory.
    pub async fn list_data_hashes(&self) -> Result<Vec<String>> {
        use futures::TryStreamExt;

        let prefix = ObjectPath::from("data/");
        let stream = self.inner.list(Some(&prefix));

        let items: Vec<_> = stream.try_collect().await?;

        let hashes = items
            .into_iter()
            .filter_map(|meta| {
                let path = meta.location.as_ref();
                path.strip_prefix("data/").map(|s| s.to_string())
            })
            .collect();

        Ok(hashes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_storage() {
        let storage = Storage::memory();

        let hash = "abc123";
        let data = Bytes::from("hello world");

        // Put and get data
        storage.put_data(hash, data.clone()).await.unwrap();
        let retrieved = storage.get_data(hash).await.unwrap().unwrap();
        assert_eq!(retrieved, data);

        // Check existence
        assert!(storage.has_data(hash).await.unwrap());

        // List hashes
        let hashes = storage.list_data_hashes().await.unwrap();
        assert_eq!(hashes.len(), 1);
        assert_eq!(hashes[0], hash);

        // Delete
        storage.delete_data(hash).await.unwrap();
        assert!(!storage.has_data(hash).await.unwrap());
    }

    #[tokio::test]
    async fn test_local_storage() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = ObjectStoreConfig::Local {
            path: temp_dir.path().to_path_buf(),
        };

        let storage = Storage::new(config).await.unwrap();

        let hash = "def456";
        let data = Bytes::from("test data");

        storage.put_data(hash, data.clone()).await.unwrap();
        let retrieved = storage.get_data(hash).await.unwrap().unwrap();
        assert_eq!(retrieved, data);

        // Verify file exists on disk
        let file_path = temp_dir.path().join("data").join(hash);
        assert!(file_path.exists());
    }

    #[tokio::test]
    async fn test_outboard_storage() {
        let storage = Storage::memory();

        let hash = "xyz789";
        let outboard = Bytes::from("outboard data");

        storage.put_outboard(hash, outboard.clone()).await.unwrap();
        let retrieved = storage.get_outboard(hash).await.unwrap().unwrap();
        assert_eq!(retrieved, outboard);

        storage.delete_outboard(hash).await.unwrap();
        assert!(storage.get_outboard(hash).await.unwrap().is_none());
    }
}
