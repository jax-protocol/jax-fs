//! Actor for handling blob store commands.
//!
//! This actor processes commands from the iroh-blobs API and coordinates
//! between SQLite (metadata) and object storage (data storage).
//!
//! NOTE: This is a work-in-progress implementation. The full iroh-blobs
//! Command/ApiClient integration is complex. For now, we provide a simpler
//! direct API that can be used alongside the store.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use bytes::Bytes;
use iroh_blobs::{api::blobs::Bitfield, Hash, HashAndFormat};
use sqlx::Row;
use tokio::sync::watch;
use tracing::info;

use super::bao_file::{BaoFileStorage, CompleteStorage, raw_outboard_size};
use super::entry_state::needs_outboard;
use super::minio::BlobObjectStore;
use crate::daemon::database::Database;

/// State for a single blob entry.
#[derive(Clone)]
pub struct BaoFileHandle {
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

    /// Get the current bitfield.
    pub fn bitfield(&self) -> Bitfield {
        self.state.borrow().bitfield()
    }

    /// Get the hash.
    pub fn hash(&self) -> Hash {
        self.hash
    }
}

/// Internal state of the blob store.
struct State {
    /// Map of hash -> handle for each blob
    data: HashMap<Hash, BaoFileHandle>,
    /// Tags mapping names to hashes
    tags: BTreeMap<String, HashAndFormat>,
    /// Handle for the empty hash (special case)
    empty_hash: BaoFileHandle,
}

/// Direct API for the blob store (simpler than full iroh-blobs Command interface).
pub struct BlobStoreApi {
    state: tokio::sync::RwLock<State>,
    db: Database,
    store: Arc<BlobObjectStore>,
}

impl std::fmt::Debug for BlobStoreApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlobStoreApi").finish()
    }
}

impl BlobStoreApi {
    /// Create a new blob store API.
    pub async fn new(
        db: Database,
        store: BlobObjectStore,
    ) -> Result<Self, crate::daemon::database::blobs_store::StoreError> {
        let store = Arc::new(store);

        let api = Self {
            state: tokio::sync::RwLock::new(State {
                data: HashMap::new(),
                tags: BTreeMap::new(),
                empty_hash: BaoFileHandle::new_complete(Hash::EMPTY, 0),
            }),
            db,
            store,
        };

        // Load existing blobs from database
        api.load_from_db().await?;

        Ok(api)
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
    pub async fn put(
        &self,
        data: Bytes,
    ) -> Result<Hash, crate::daemon::database::blobs_store::StoreError> {
        use bao_tree::blake3;
        use std::time::{SystemTime, UNIX_EPOCH};

        let size = data.len() as u64;

        // Compute the BLAKE3 hash
        let hash_bytes = blake3::hash(&data);
        let hash = Hash::from(*hash_bytes.as_bytes());
        let hash_str = hash.to_hex().to_string();

        // Store data in object storage
        self.store.put_data(&hash_str, data.clone()).await?;

        // Compute and store outboard if needed
        let has_outboard = if needs_outboard(size) {
            // For now, create a placeholder outboard
            // TODO: Proper BAO outboard computation
            let outboard_size = raw_outboard_size(size);
            let outboard = vec![0u8; outboard_size as usize];
            self.store
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
    pub async fn get(
        &self,
        hash: &Hash,
    ) -> Result<Option<Bytes>, crate::daemon::database::blobs_store::StoreError> {
        let hash_str = hash.to_hex().to_string();

        // Check if we have it
        {
            let state = self.state.read().await;
            if !state.data.contains_key(hash) {
                return Ok(None);
            }
        }

        // Fetch from object storage
        match self.store.get_data(&hash_str).await {
            Ok(data) => Ok(Some(data)),
            Err(super::minio::ObjectStoreError::Store(e))
                if e.to_string().contains("not found") =>
            {
                Ok(None)
            }
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
    pub async fn delete(
        &self,
        hash: &Hash,
    ) -> Result<bool, crate::daemon::database::blobs_store::StoreError> {
        let hash_str = hash.to_hex().to_string();

        // Remove from in-memory state
        {
            let mut state = self.state.write().await;
            if state.data.remove(hash).is_none() {
                return Ok(false);
            }
        }

        // Delete from object storage
        let _ = self.store.delete_data(&hash_str).await;
        let _ = self.store.delete_outboard(&hash_str).await;

        // Delete from SQLite
        sqlx::query("DELETE FROM blobs WHERE hash = ?")
            .bind(&hash_str)
            .execute(&*self.db)
            .await?;

        Ok(true)
    }

    /// Get the object store client.
    pub fn store(&self) -> &BlobObjectStore {
        &self.store
    }
}

/// Status of a blob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlobStatus {
    Complete { size: u64 },
    Partial { size: Option<u64> },
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
