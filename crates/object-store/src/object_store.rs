//! ObjectStore - unified iroh-blobs Store implementation backed by SQLite + object storage.
//!
//! This module merges the BlobStore (content-addressed storage with SQLite metadata)
//! and the iroh-blobs Store adapter into a single type. It provides both direct
//! constructors and conversion to iroh_blobs::api::Store for P2P sync.

use std::ops::Deref;
use std::path::Path;

use bytes::Bytes;
use iroh_blobs::api::proto::Command;
use iroh_blobs::Hash;
use tracing::{debug, info, warn};

use crate::actor::ObjectStoreActor;
use crate::database::Database;
use crate::error::Result;
use crate::storage::{ObjectStoreConfig, Storage};

/// Size threshold for generating BAO outboard data (16KB).
/// Blobs larger than this will have outboard verification data stored separately.
const OUTBOARD_THRESHOLD: usize = 16 * 1024;

/// Type alias for the irpc client
type ApiClient = irpc::Client<iroh_blobs::api::proto::Request>;

/// Internal BlobStore combining SQLite metadata with object storage.
///
/// This is used internally by ObjectStore and the actor.
#[derive(Debug, Clone)]
pub(crate) struct BlobStore {
    db: Database,
    storage: Storage,
}

impl BlobStore {
    /// Create a new BlobStore with a file-based SQLite database.
    pub async fn new(db_path: &Path, config: ObjectStoreConfig) -> Result<Self> {
        let db = Database::new(db_path).await?;
        let storage = Storage::new(config).await?;
        Ok(Self { db, storage })
    }

    /// Create a new BlobStore with an in-memory SQLite database.
    pub async fn in_memory(config: ObjectStoreConfig) -> Result<Self> {
        let db = Database::in_memory().await?;
        let storage = Storage::new(config).await?;
        Ok(Self { db, storage })
    }

    /// Create a new BlobStore backed by local filesystem.
    pub async fn new_local(data_dir: &Path) -> Result<Self> {
        let db_path = data_dir.join("blobs.db");
        let objects_path = data_dir.join("objects");
        let config = ObjectStoreConfig::Local { path: objects_path };
        Self::new(&db_path, config).await
    }

    /// Create a fully ephemeral BlobStore (in-memory DB + in-memory object storage).
    pub async fn new_ephemeral() -> Result<Self> {
        Self::in_memory(ObjectStoreConfig::Memory).await
    }

    /// Store data and return its content hash.
    pub async fn put(&self, data: Vec<u8>) -> Result<Hash> {
        let size = data.len();
        let hash = Hash::new(&data);
        let hash_str = hash.to_string();

        debug!(hash = %hash_str, size = size, "storing blob");

        let has_outboard = size > OUTBOARD_THRESHOLD;
        self.storage.put_data(&hash_str, Bytes::from(data)).await?;
        self.db
            .insert_blob(&hash_str, size as i64, has_outboard)
            .await?;

        info!(hash = %hash_str, size = size, "blob stored successfully");
        Ok(hash)
    }

    /// Retrieve blob data by hash.
    pub async fn get(&self, hash: &Hash) -> Result<Option<Bytes>> {
        let hash_str = hash.to_string();
        if !self.db.has_blob(&hash_str).await? {
            return Ok(None);
        }
        self.storage.get_data(&hash_str).await
    }

    /// Delete a blob from the store.
    pub async fn delete(&self, hash: &Hash) -> Result<bool> {
        let hash_str = hash.to_string();

        let metadata = self.db.get_blob(&hash_str).await?;
        if metadata.is_none() {
            return Ok(false);
        }

        let metadata = metadata.unwrap();
        self.storage.delete_data(&hash_str).await?;
        if metadata.has_outboard {
            self.storage.delete_outboard(&hash_str).await?;
        }
        self.db.delete_blob(&hash_str).await?;

        info!(hash = %hash_str, "blob deleted");
        Ok(true)
    }

    /// List all blob hashes in the store.
    pub async fn list(&self) -> Result<Vec<Hash>> {
        let hash_strings = self.db.list_blobs().await?;
        let mut hashes = Vec::with_capacity(hash_strings.len());

        for s in hash_strings {
            match s.parse::<Hash>() {
                Ok(h) => hashes.push(h),
                Err(_) => {
                    warn!(hash = %s, "invalid hash in database, skipping");
                }
            }
        }

        Ok(hashes)
    }
}

/// ObjectStore provides an iroh-blobs compatible store backed by SQLite + object storage.
///
/// This store can be used with iroh-blobs' BlobsProtocol to enable P2P sync
/// while storing blobs in S3/MinIO/local filesystem/memory.
///
/// # Example
///
/// ```rust,no_run
/// use jax_object_store::ObjectStore;
/// use std::path::Path;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create a store with local filesystem storage
/// let store = ObjectStore::new_local(Path::new("/tmp/blobs")).await?;
///
/// // Convert to iroh_blobs::api::Store for use with BlobsProtocol
/// let iroh_store: iroh_blobs::api::Store = store.into();
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct ObjectStore {
    client: ApiClient,
}

impl ObjectStore {
    /// Create a new ObjectStore with the given configuration.
    ///
    /// # Arguments
    /// * `db_path` - Path to the SQLite database file
    /// * `config` - Object storage configuration (S3, MinIO, local, or memory)
    pub async fn new(db_path: &Path, config: ObjectStoreConfig) -> Result<Self> {
        let store = BlobStore::new(db_path, config).await?;
        Ok(Self::from_blob_store(store))
    }

    /// Create a new ObjectStore backed by local filesystem.
    ///
    /// This creates both SQLite DB and object storage in the given directory.
    ///
    /// # Arguments
    /// * `data_dir` - Directory for all storage (db at data_dir/blobs.db, objects at data_dir/objects/)
    pub async fn new_local(data_dir: &Path) -> Result<Self> {
        let store = BlobStore::new_local(data_dir).await?;
        Ok(Self::from_blob_store(store))
    }

    /// Create a new ObjectStore with S3/MinIO storage.
    ///
    /// # Arguments
    /// * `db_path` - Path to the SQLite database file
    /// * `endpoint` - S3 endpoint URL (e.g., "http://localhost:9000" for MinIO)
    /// * `access_key` - S3 access key ID
    /// * `secret_key` - S3 secret access key
    /// * `bucket` - S3 bucket name
    /// * `region` - Optional S3 region (defaults to "us-east-1")
    pub async fn new_s3(
        db_path: &Path,
        endpoint: &str,
        access_key: &str,
        secret_key: &str,
        bucket: &str,
        region: Option<&str>,
    ) -> Result<Self> {
        let config = ObjectStoreConfig::S3 {
            endpoint: endpoint.to_string(),
            access_key: access_key.to_string(),
            secret_key: secret_key.to_string(),
            bucket: bucket.to_string(),
            region: region.map(|s| s.to_string()),
        };
        Self::new(db_path, config).await
    }

    /// Create a fully ephemeral ObjectStore (in-memory DB + in-memory object storage).
    ///
    /// Data will be lost when the ObjectStore is dropped. Useful for testing.
    pub async fn new_ephemeral() -> Result<Self> {
        let store = BlobStore::new_ephemeral().await?;
        Ok(Self::from_blob_store(store))
    }

    /// Create an ObjectStore from an existing BlobStore.
    fn from_blob_store(store: BlobStore) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel::<Command>(256);
        let actor = ObjectStoreActor::new(store, rx);
        tokio::spawn(actor.run());
        let client: ApiClient = tx.into();
        Self { client }
    }

    /// Convert to an iroh_blobs::api::Store.
    ///
    /// This method uses unsafe transmute because:
    /// - Store is repr(transparent) over ApiClient
    /// - ApiClient = irpc::Client<proto::Request>
    /// - Our client field is the same type
    /// - Therefore the memory layouts are identical
    pub fn into_iroh_store(self) -> iroh_blobs::api::Store {
        // SAFETY: iroh_blobs::api::Store is repr(transparent) over ApiClient
        // and our ObjectStore::client is of type ApiClient. Since Store wraps
        // ApiClient with repr(transparent), they have the same memory layout.
        unsafe { std::mem::transmute::<ApiClient, iroh_blobs::api::Store>(self.client) }
    }

    /// Get a reference to the store as iroh_blobs::api::Store.
    fn as_iroh_store(&self) -> &iroh_blobs::api::Store {
        // SAFETY: Same reasoning as into_iroh_store - Store is repr(transparent)
        // over ApiClient, so &ApiClient can be safely reinterpreted as &Store.
        unsafe { std::mem::transmute::<&ApiClient, &iroh_blobs::api::Store>(&self.client) }
    }
}

/// Convert ObjectStore to iroh_blobs::api::Store.
///
/// This allows ObjectStore to be used with BlobsProtocol for P2P sync.
impl From<ObjectStore> for iroh_blobs::api::Store {
    fn from(value: ObjectStore) -> Self {
        value.into_iroh_store()
    }
}

/// Deref to iroh_blobs::api::Store for convenient API access.
impl Deref for ObjectStore {
    type Target = iroh_blobs::api::Store;

    fn deref(&self) -> &Self::Target {
        self.as_iroh_store()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroh_blobs::api::blobs::BlobStatus;

    /// Statistics from a recovery operation.
    #[derive(Debug, Default)]
    struct RecoveryStats {
        found: usize,
        added: usize,
        existing: usize,
        errors: usize,
    }

    /// Test-only methods on BlobStore for verifying internal state.
    impl BlobStore {
        async fn has(&self, hash: &Hash) -> Result<bool> {
            let hash_str = hash.to_string();
            self.db.has_blob(&hash_str).await
        }

        async fn count(&self) -> Result<u64> {
            let count = self.db.count_blobs().await?;
            Ok(count as u64)
        }

        async fn total_size(&self) -> Result<u64> {
            let size = self.db.total_size().await?;
            Ok(size as u64)
        }

        async fn recover_from_storage(&self) -> Result<RecoveryStats> {
            let mut stats = RecoveryStats::default();
            let hashes = self.storage.list_data_hashes().await?;
            stats.found = hashes.len();

            for hash_str in hashes {
                if self.db.has_blob(&hash_str).await? {
                    stats.existing += 1;
                    continue;
                }

                match self.storage.get_data(&hash_str).await {
                    Ok(Some(data)) => {
                        let size = data.len();
                        let has_outboard = size > OUTBOARD_THRESHOLD;

                        if let Err(e) = self
                            .db
                            .insert_blob(&hash_str, size as i64, has_outboard)
                            .await
                        {
                            warn!(hash = %hash_str, error = %e, "failed to insert recovered blob metadata");
                            stats.errors += 1;
                        } else {
                            debug!(hash = %hash_str, size = size, "recovered blob metadata");
                            stats.added += 1;
                        }
                    }
                    Ok(None) => {
                        warn!(hash = %hash_str, "blob listed but not found in storage");
                        stats.errors += 1;
                    }
                    Err(e) => {
                        warn!(hash = %hash_str, error = %e, "failed to read blob during recovery");
                        stats.errors += 1;
                    }
                }
            }

            Ok(stats)
        }
    }

    #[tokio::test]
    async fn test_ephemeral_store() {
        let store = ObjectStore::new_ephemeral().await.unwrap();

        let data = b"hello world".to_vec();
        let tt = store.add_bytes(data.clone()).temp_tag().await.unwrap();
        let hash = tt.hash();

        let status = store.status(hash).await.unwrap();
        assert!(matches!(status, BlobStatus::Complete { size: 11 }));

        let retrieved = store.get_bytes(hash).await.unwrap();
        assert_eq!(retrieved.as_ref(), data.as_slice());
    }

    #[tokio::test]
    async fn test_local_store() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new_local(temp_dir.path()).await.unwrap();

        let data = b"test local storage".to_vec();
        let tt = store.add_bytes(data.clone()).temp_tag().await.unwrap();
        let hash = tt.hash();

        let status = store.status(hash).await.unwrap();
        assert!(matches!(status, BlobStatus::Complete { .. }));

        let retrieved = store.get_bytes(hash).await.unwrap();
        assert_eq!(retrieved.as_ref(), data.as_slice());
    }

    #[tokio::test]
    async fn test_list_blobs() {
        let store = ObjectStore::new_ephemeral().await.unwrap();

        let _tt1 = store
            .add_bytes(b"blob one".to_vec())
            .temp_tag()
            .await
            .unwrap();
        let _tt2 = store
            .add_bytes(b"blob two".to_vec())
            .temp_tag()
            .await
            .unwrap();

        use n0_future::StreamExt;
        let stream = store.list().stream().await.unwrap();
        let blobs: Vec<_> = stream.collect().await;
        assert_eq!(blobs.len(), 2);
    }

    #[tokio::test]
    async fn test_tags() {
        let store = ObjectStore::new_ephemeral().await.unwrap();

        let data = b"tagged blob".to_vec();
        let tt = store.add_bytes(data.clone()).temp_tag().await.unwrap();
        let hash = tt.hash();

        let tag = store.tags().create(tt.hash_and_format()).await.unwrap();

        use n0_future::StreamExt;
        let stream = store.tags().list().await.unwrap();
        let tags: Vec<_> = stream.collect().await;
        assert_eq!(tags.len(), 1);
        let first_tag = tags[0].as_ref().unwrap();
        assert_eq!(first_tag.name, tag);
        assert_eq!(first_tag.hash, hash);
    }

    #[tokio::test]
    async fn test_convert_to_iroh_store() {
        let obj_store = ObjectStore::new_ephemeral().await.unwrap();

        let iroh_store: iroh_blobs::api::Store = obj_store.into();

        let data = b"test via iroh store".to_vec();
        let tt = iroh_store.add_bytes(data.clone()).temp_tag().await.unwrap();
        let hash = tt.hash();

        let retrieved = iroh_store.get_bytes(hash).await.unwrap();
        assert_eq!(retrieved.as_ref(), data.as_slice());
    }

    #[tokio::test]
    async fn test_blob_store_ephemeral() {
        let store = BlobStore::new_ephemeral().await.unwrap();

        let data = b"hello world".to_vec();
        let hash = store.put(data.clone()).await.unwrap();

        assert!(store.has(&hash).await.unwrap());

        let retrieved = store.get(&hash).await.unwrap().unwrap();
        assert_eq!(retrieved.as_ref(), data.as_slice());

        let list = store.list().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0], hash);

        assert_eq!(store.count().await.unwrap(), 1);
        assert_eq!(store.total_size().await.unwrap(), data.len() as u64);

        assert!(store.delete(&hash).await.unwrap());
        assert!(!store.has(&hash).await.unwrap());
        assert_eq!(store.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_blob_store_local() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = BlobStore::new_local(temp_dir.path()).await.unwrap();

        let data = b"test local storage".to_vec();
        let hash = store.put(data.clone()).await.unwrap();

        assert!(temp_dir.path().join("blobs.db").exists());
        assert!(temp_dir
            .path()
            .join("objects")
            .join("data")
            .join(hash.to_string())
            .exists());

        let retrieved = store.get(&hash).await.unwrap().unwrap();
        assert_eq!(retrieved.as_ref(), data.as_slice());
    }

    #[tokio::test]
    async fn test_blob_store_recovery() {
        let temp_dir = tempfile::tempdir().unwrap();

        let hash1;
        let hash2;

        {
            let store = BlobStore::new_local(temp_dir.path()).await.unwrap();
            hash1 = store.put(b"blob one".to_vec()).await.unwrap();
            hash2 = store.put(b"blob two".to_vec()).await.unwrap();
        }

        tokio::fs::remove_file(temp_dir.path().join("blobs.db"))
            .await
            .unwrap();

        let store = BlobStore::new_local(temp_dir.path()).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 0);

        let stats = store.recover_from_storage().await.unwrap();
        assert_eq!(stats.found, 2);
        assert_eq!(stats.added, 2);
        assert_eq!(stats.existing, 0);
        assert_eq!(stats.errors, 0);

        assert!(store.has(&hash1).await.unwrap());
        assert!(store.has(&hash2).await.unwrap());
        assert_eq!(store.count().await.unwrap(), 2);

        let stats2 = store.recover_from_storage().await.unwrap();
        assert_eq!(stats2.found, 2);
        assert_eq!(stats2.added, 0);
        assert_eq!(stats2.existing, 2);
    }

    #[tokio::test]
    async fn test_blob_store_get_nonexistent() {
        let store = BlobStore::new_ephemeral().await.unwrap();
        let fake_hash = Hash::new(b"this data was never stored");

        assert!(!store.has(&fake_hash).await.unwrap());
        assert!(store.get(&fake_hash).await.unwrap().is_none());
        assert!(!store.delete(&fake_hash).await.unwrap());
    }

    #[tokio::test]
    async fn test_blob_store_multiple_blobs() {
        let store = BlobStore::new_ephemeral().await.unwrap();

        let blobs: Vec<Vec<u8>> = vec![
            b"first blob".to_vec(),
            b"second blob".to_vec(),
            b"third blob".to_vec(),
        ];

        let mut hashes = Vec::new();
        for data in &blobs {
            let hash = store.put(data.clone()).await.unwrap();
            hashes.push(hash);
        }

        assert_eq!(store.count().await.unwrap(), 3);

        for hash in &hashes {
            assert!(store.has(hash).await.unwrap());
        }

        assert!(store.delete(&hashes[1]).await.unwrap());
        assert_eq!(store.count().await.unwrap(), 2);
        assert!(!store.has(&hashes[1]).await.unwrap());

        assert!(store.has(&hashes[0]).await.unwrap());
        assert!(store.has(&hashes[2]).await.unwrap());
    }
}
