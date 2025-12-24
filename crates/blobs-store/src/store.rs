//! Main blob store implementation.
//!
//! Provides a high-level API for storing and retrieving blobs using
//! object storage for data and SQLite for metadata.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use iroh_blobs::{api::blobs::Bitfield, Hash, HashAndFormat};
use sqlx::Row;
use tokio::sync::watch;
use tracing::info;

use crate::bao_file::{raw_outboard_size, BaoFileStorage, CompleteStorage};
use crate::database::Database;
use crate::entry_state::needs_outboard;
use crate::object_store::{BlobObjectStore, ObjectStoreConfig, ObjectStoreError};
use crate::RecoveryStats;

/// Errors that can occur when using the blob store.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("object store error: {0}")]
    ObjectStore(#[from] ObjectStoreError),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("database setup error: {0}")]
    DatabaseSetup(#[from] crate::database::DatabaseError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// State for a single blob entry.
#[derive(Clone)]
struct BaoFileHandle {
    hash: Hash,
    state: Arc<watch::Sender<BaoFileStorage>>,
}

impl BaoFileHandle {
    fn new_partial(hash: Hash) -> Self {
        let hash_str = hash.to_hex().to_string();
        Self {
            hash,
            state: Arc::new(watch::Sender::new(BaoFileStorage::new_partial_mem(
                hash_str, None,
            ))),
        }
    }

    fn new_complete(hash: Hash, size: u64) -> Self {
        let hash_str = hash.to_hex().to_string();
        Self {
            hash,
            state: Arc::new(watch::Sender::new(BaoFileStorage::Complete(
                CompleteStorage::new(hash_str, size),
            ))),
        }
    }

    fn bitfield(&self) -> Bitfield {
        self.state.borrow().bitfield()
    }

    #[allow(dead_code)]
    fn hash(&self) -> Hash {
        self.hash
    }
}

/// Internal state of the blob store.
struct State {
    /// Map of hash -> handle for each blob
    data: HashMap<Hash, BaoFileHandle>,
    /// Tags mapping names to hashes
    #[allow(dead_code)]
    tags: BTreeMap<String, HashAndFormat>,
    /// Handle for the empty hash (special case)
    #[allow(dead_code)]
    empty_hash: BaoFileHandle,
}

/// A blob store backed by object storage and SQLite.
///
/// - **Object storage** (S3/MinIO/GCS/Azure/local) stores all blob data
/// - **SQLite** stores metadata (hash, size, state, tags)
///
/// The SQLite database can be in-memory or file-based. If using in-memory,
/// metadata can be recovered from object storage on restart.
pub struct BlobStore {
    state: tokio::sync::RwLock<State>,
    db: Database,
    object_store: Arc<BlobObjectStore>,
}

impl std::fmt::Debug for BlobStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlobStore").finish()
    }
}

impl BlobStore {
    /// Create a new blob store with a file-based SQLite database.
    ///
    /// # Arguments
    /// * `db_path` - Path to the SQLite database file
    /// * `object_store_config` - Configuration for the object store
    pub async fn new(
        db_path: impl AsRef<Path>,
        object_store_config: ObjectStoreConfig,
    ) -> Result<Self, StoreError> {
        let db = Database::new(db_path).await?;
        let object_store = BlobObjectStore::new_s3(object_store_config)?;
        Self::from_parts(db, object_store).await
    }

    /// Create a new blob store with an in-memory SQLite database.
    ///
    /// Useful for testing or when metadata can be recovered from object storage.
    pub async fn in_memory(object_store_config: ObjectStoreConfig) -> Result<Self, StoreError> {
        let db = Database::in_memory().await?;
        let object_store = BlobObjectStore::new_s3(object_store_config)?;
        Self::from_parts(db, object_store).await
    }

    /// Create a new blob store using the local filesystem for storage.
    ///
    /// This is useful for local development and testing without requiring
    /// S3/MinIO. Both the SQLite database and blob data are stored in the
    /// specified directory.
    ///
    /// # Arguments
    /// * `data_dir` - Directory for storing blobs and SQLite database
    ///
    /// # Example
    /// ```rust,ignore
    /// let store = BlobStore::new_local("./data/blobs").await?;
    /// ```
    pub async fn new_local(data_dir: impl AsRef<Path>) -> Result<Self, StoreError> {
        let data_dir = data_dir.as_ref();

        // Create directory if it doesn't exist
        std::fs::create_dir_all(data_dir)?;

        // SQLite database in the data directory
        let db_path = data_dir.join("blobs.db");
        let db = Database::new(&db_path).await?;

        // Object storage in a subdirectory
        let objects_dir = data_dir.join("objects");
        let object_store = BlobObjectStore::new_local(&objects_dir)?;

        Self::from_parts(db, object_store).await
    }

    /// Create a fully in-memory blob store (both SQLite and object storage).
    ///
    /// This is useful for unit testing. All data is lost when the store is dropped.
    pub async fn new_ephemeral() -> Result<Self, StoreError> {
        let db = Database::in_memory().await?;
        let object_store = BlobObjectStore::new_in_memory();
        Self::from_parts(db, object_store).await
    }

    /// Create a blob store from existing database and object store.
    ///
    /// This is useful for testing with in-memory object stores.
    pub async fn from_parts(
        db: Database,
        object_store: BlobObjectStore,
    ) -> Result<Self, StoreError> {
        let store = Self {
            state: tokio::sync::RwLock::new(State {
                data: HashMap::new(),
                tags: BTreeMap::new(),
                empty_hash: BaoFileHandle::new_complete(Hash::EMPTY, 0),
            }),
            db,
            object_store: Arc::new(object_store),
        };

        // Load existing blobs from database
        store.load_from_db().await?;

        Ok(store)
    }

    /// Load existing blobs from the database.
    async fn load_from_db(&self) -> Result<(), sqlx::Error> {
        let rows = sqlx::query("SELECT hash, size, state FROM blobs")
            .fetch_all(&*self.db)
            .await?;

        let mut state = self.state.write().await;
        for row in rows {
            let hash_hex: String = row.get("hash");
            let hash_bytes = hex::decode(&hash_hex).unwrap_or_default();
            if hash_bytes.len() != 32 {
                continue;
            }
            let mut hash_arr = [0u8; 32];
            hash_arr.copy_from_slice(&hash_bytes);
            let hash = Hash::from_bytes(hash_arr);

            let size: i64 = row.get("size");
            let state_str: String = row.get("state");

            let handle = if state_str == "complete" {
                BaoFileHandle::new_complete(hash, size as u64)
            } else {
                BaoFileHandle::new_partial(hash)
            };

            state.data.insert(hash, handle);
        }

        info!("loaded {} blobs from database", state.data.len());
        Ok(())
    }

    /// Store bytes in the blob store.
    ///
    /// Returns the BLAKE3 hash of the stored data.
    pub async fn put(&self, data: Bytes) -> Result<Hash, StoreError> {
        use bao_tree::blake3;

        let size = data.len() as u64;

        // Compute the BLAKE3 hash
        let hash_bytes = blake3::hash(&data);
        let hash = Hash::from(*hash_bytes.as_bytes());
        let hash_str = hash.to_hex().to_string();

        // Store data in object storage
        self.object_store.put_data(&hash_str, data.clone()).await?;

        // Compute and store outboard if needed
        let has_outboard = if needs_outboard(size) {
            // For now, create a placeholder outboard
            // TODO: Proper BAO outboard computation
            let outboard_size = raw_outboard_size(size);
            let outboard = vec![0u8; outboard_size as usize];
            self.object_store
                .put_outboard(&hash_str, Bytes::from(outboard))
                .await?;
            true
        } else {
            false
        };

        // Update SQLite metadata
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        sqlx::query(
            r#"
            INSERT INTO blobs (hash, size, has_outboard, state, created_at, updated_at)
            VALUES (?, ?, ?, 'complete', ?, ?)
            ON CONFLICT(hash) DO UPDATE SET
                size = excluded.size,
                has_outboard = excluded.has_outboard,
                state = 'complete',
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&hash_str)
        .bind(size as i64)
        .bind(has_outboard)
        .bind(now)
        .bind(now)
        .execute(&*self.db)
        .await?;

        // Update in-memory state
        {
            let mut state = self.state.write().await;
            let handle = BaoFileHandle::new_complete(hash, size);
            state.data.insert(hash, handle);
        }

        info!("stored blob {} ({} bytes)", hash_str, size);
        Ok(hash)
    }

    /// Get bytes from the blob store.
    ///
    /// Returns `None` if the blob doesn't exist.
    pub async fn get(&self, hash: &Hash) -> Result<Option<Bytes>, StoreError> {
        let hash_str = hash.to_hex().to_string();

        // Check if we have it
        {
            let state = self.state.read().await;
            if !state.data.contains_key(hash) {
                return Ok(None);
            }
        }

        // Fetch from object storage
        match self.object_store.get_data(&hash_str).await {
            Ok(data) => Ok(Some(data)),
            Err(ObjectStoreError::Store(e)) if e.to_string().contains("not found") => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Check if a blob exists.
    pub async fn has(&self, hash: &Hash) -> bool {
        let state = self.state.read().await;
        state.data.contains_key(hash)
    }

    /// Get blob status.
    pub async fn status(&self, hash: &Hash) -> Option<BlobStatus> {
        let state = self.state.read().await;
        state.data.get(hash).map(|handle| {
            let bitfield = handle.bitfield();
            if bitfield.is_complete() {
                BlobStatus::Complete {
                    size: bitfield.size(),
                }
            } else {
                BlobStatus::Partial {
                    size: bitfield.validated_size(),
                }
            }
        })
    }

    /// List all blob hashes.
    pub async fn list(&self) -> Vec<Hash> {
        let state = self.state.read().await;
        state.data.keys().cloned().collect()
    }

    /// Delete a blob.
    ///
    /// Returns `true` if the blob was deleted, `false` if it didn't exist.
    pub async fn delete(&self, hash: &Hash) -> Result<bool, StoreError> {
        let hash_str = hash.to_hex().to_string();

        // Remove from in-memory state
        {
            let mut state = self.state.write().await;
            if state.data.remove(hash).is_none() {
                return Ok(false);
            }
        }

        // Delete from object storage
        let _ = self.object_store.delete_data(&hash_str).await;
        let _ = self.object_store.delete_outboard(&hash_str).await;

        // Delete from SQLite
        sqlx::query("DELETE FROM blobs WHERE hash = ?")
            .bind(&hash_str)
            .execute(&*self.db)
            .await?;

        Ok(true)
    }

    /// Get the underlying object store client.
    pub fn object_store(&self) -> &BlobObjectStore {
        &self.object_store
    }

    /// Get the underlying database.
    pub fn database(&self) -> &Database {
        &self.db
    }

    /// Recover metadata from object storage.
    ///
    /// This scans all objects in storage and rebuilds the SQLite metadata.
    /// Useful for disaster recovery or when using an in-memory database.
    pub async fn recover_from_storage(&self) -> Result<RecoveryStats, StoreError> {
        let mut stats = RecoveryStats::default();

        // List all complete blob hashes
        let hashes = self.object_store.list_data_hashes().await?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        for hash_str in hashes {
            // Check if outboard exists
            let has_outboard = self.object_store.has_outboard(&hash_str).await;

            // Get size from object metadata
            let path = object_store::path::Path::from(format!("data/{}", hash_str));
            let size = match self.object_store.head(&path).await {
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
            .execute(&*self.db)
            .await;

            if result.is_ok() {
                // Also update in-memory state
                let hash_bytes = hex::decode(&hash_str).unwrap_or_default();
                if hash_bytes.len() == 32 {
                    let mut hash_arr = [0u8; 32];
                    hash_arr.copy_from_slice(&hash_bytes);
                    let hash = Hash::from_bytes(hash_arr);

                    let mut state = self.state.write().await;
                    state
                        .data
                        .insert(hash, BaoFileHandle::new_complete(hash, size as u64));
                }

                stats.complete_blobs += 1;
            }
        }

        info!(
            "recovered {} blobs from object storage",
            stats.complete_blobs
        );
        Ok(stats)
    }
}

/// Status of a blob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlobStatus {
    /// Blob is complete and available.
    Complete {
        /// Size in bytes.
        size: u64,
    },
    /// Blob is partially available.
    Partial {
        /// Size of validated data, if known.
        size: Option<u64>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blob_status() {
        let status = BlobStatus::Complete { size: 1024 };
        assert!(matches!(status, BlobStatus::Complete { .. }));
    }
}
