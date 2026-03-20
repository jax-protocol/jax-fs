pub mod actor;
pub mod database;
pub mod store;
pub mod transform;

use std::path::Path;

use bytes::Bytes;
use tokio::sync::mpsc;

use actor::{CacheActor, CacheHint};
use database::{CacheDatabase, InsertEntry};
use store::CacheStore;

/// Configuration for the gateway cache.
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// How many old heights to retain per bucket (default: 1).
    pub max_versions: u32,
    /// Total size limit for layer 2 content in bytes (default: 1 GB).
    pub max_cache_size_bytes: Option<u64>,
    /// TTL for layer 1 entries in seconds (default: none).
    pub max_entry_age_secs: Option<u64>,
    /// How often the actor runs its eviction sweep in seconds (default: 300).
    pub eviction_interval_secs: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_versions: 1,
            max_cache_size_bytes: Some(1024 * 1024 * 1024), // 1 GB
            max_entry_age_secs: None,
            eviction_interval_secs: 300,
        }
    }
}

/// Handle for the gateway response cache.
///
/// Provides direct read access (no actor round-trip) and async writes.
/// The background actor handles eviction and cleanup.
#[derive(Clone)]
pub struct GatewayCache {
    db: CacheDatabase,
    store: CacheStore,
    hints_tx: mpsc::Sender<CacheHint>,
}

impl GatewayCache {
    /// Initialize the cache with filesystem storage at the given data directory.
    pub async fn new(data_dir: &Path, config: CacheConfig) -> Result<Self, GatewayCacheError> {
        let cache_dir = data_dir.join("gateway-cache");
        tokio::fs::create_dir_all(&cache_dir)
            .await
            .map_err(GatewayCacheError::Io)?;

        let db = CacheDatabase::open(&cache_dir.join("cache.db"))
            .await
            .map_err(GatewayCacheError::Database)?;
        let store = CacheStore::new_local(&cache_dir.join("blobs"))
            .await
            .map_err(GatewayCacheError::Store)?;

        let (hints_tx, hints_rx) = mpsc::channel(64);

        // Spawn the background eviction actor
        let actor = CacheActor::new(db.clone(), store.clone(), config, hints_rx);
        tokio::spawn(actor.run());

        Ok(Self {
            db,
            store,
            hints_tx,
        })
    }

    /// Create an in-memory cache (for tests).
    pub async fn new_memory(config: CacheConfig) -> Result<Self, GatewayCacheError> {
        let db = CacheDatabase::in_memory()
            .await
            .map_err(GatewayCacheError::Database)?;
        let store = CacheStore::new_memory();

        let (hints_tx, hints_rx) = mpsc::channel(64);

        let actor = CacheActor::new(db.clone(), store.clone(), config, hints_rx);
        tokio::spawn(actor.run());

        Ok(Self {
            db,
            store,
            hints_tx,
        })
    }

    /// Look up cached content. Returns (bytes, mime_type) on hit.
    pub async fn get(
        &self,
        bucket_id: &str,
        height: i64,
        path: &str,
        transform_params: &str,
    ) -> Option<(Bytes, String)> {
        // Layer 1: path index lookup
        let entry = match self
            .db
            .lookup(bucket_id, height, path, transform_params)
            .await
        {
            Ok(Some(entry)) => entry,
            Ok(None) => return None,
            Err(e) => {
                tracing::warn!("cache layer 1 lookup error: {}", e);
                return None;
            }
        };

        // Layer 2: content store lookup
        match self.store.get(&entry.content_hash).await {
            Ok(Some(data)) => Some((data, entry.mime_type)),
            Ok(None) => {
                tracing::debug!(
                    hash = entry.content_hash,
                    "cache layer 1 hit but layer 2 miss — orphaned index entry"
                );
                None
            }
            Err(e) => {
                tracing::warn!("cache layer 2 get error: {}", e);
                None
            }
        }
    }

    /// Populate the cache with content.
    pub async fn put(
        &self,
        bucket_id: &str,
        height: i64,
        path: &str,
        transform_params: &str,
        data: &[u8],
        mime_type: &str,
    ) {
        // Layer 2: store content (content-addressed, deduped automatically)
        let hash = match self.store.put(data).await {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!("cache put error (store): {}", e);
                return;
            }
        };

        // Layer 1: index the path → hash mapping
        if let Err(e) = self
            .db
            .insert(&InsertEntry {
                bucket_id,
                height,
                path,
                transform_params,
                content_hash: &hash,
                content_size: data.len() as i64,
                mime_type,
            })
            .await
        {
            tracing::warn!("cache put error (index): {}", e);
        }
    }

    /// Send a non-blocking hint to the cache actor.
    pub fn hint(&self, hint: CacheHint) {
        let _ = self.hints_tx.try_send(hint);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GatewayCacheError {
    #[error("cache database error: {0}")]
    Database(database::CacheDatabaseError),
    #[error("cache store error: {0}")]
    Store(store::CacheStoreError),
    #[error("I/O error: {0}")]
    Io(std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_full_cache_flow() {
        let cache = GatewayCache::new_memory(CacheConfig::default())
            .await
            .unwrap();

        // Miss
        let result = cache.get("bucket-1", 1, "/photo.jpg", "").await;
        assert!(result.is_none());

        // Populate
        let data = b"fake jpeg data";
        cache
            .put("bucket-1", 1, "/photo.jpg", "", data, "image/jpeg")
            .await;

        // Hit
        let (bytes, mime) = cache.get("bucket-1", 1, "/photo.jpg", "").await.unwrap();
        assert_eq!(bytes.as_ref(), data);
        assert_eq!(mime, "image/jpeg");
    }

    #[tokio::test]
    async fn test_transform_params_cache_separately() {
        let cache = GatewayCache::new_memory(CacheConfig::default())
            .await
            .unwrap();

        cache
            .put("b1", 1, "/photo.jpg", "", b"original", "image/jpeg")
            .await;
        cache
            .put("b1", 1, "/photo.jpg", "w=200", b"thumbnail", "image/jpeg")
            .await;

        let (original, _) = cache.get("b1", 1, "/photo.jpg", "").await.unwrap();
        assert_eq!(original.as_ref(), b"original");

        let (thumb, _) = cache.get("b1", 1, "/photo.jpg", "w=200").await.unwrap();
        assert_eq!(thumb.as_ref(), b"thumbnail");
    }

    #[tokio::test]
    async fn test_content_dedup_across_paths() {
        let cache = GatewayCache::new_memory(CacheConfig::default())
            .await
            .unwrap();
        let data = b"same content at different paths";

        cache.put("b1", 1, "/a.txt", "", data, "text/plain").await;
        cache.put("b1", 1, "/b.txt", "", data, "text/plain").await;

        // Both paths should hit
        assert!(cache.get("b1", 1, "/a.txt", "").await.is_some());
        assert!(cache.get("b1", 1, "/b.txt", "").await.is_some());
    }

    #[tokio::test]
    async fn test_different_heights() {
        let cache = GatewayCache::new_memory(CacheConfig::default())
            .await
            .unwrap();

        cache
            .put("b1", 1, "/file.txt", "", b"version 1", "text/plain")
            .await;
        cache
            .put("b1", 2, "/file.txt", "", b"version 2", "text/plain")
            .await;

        let (v1, _) = cache.get("b1", 1, "/file.txt", "").await.unwrap();
        assert_eq!(v1.as_ref(), b"version 1");

        let (v2, _) = cache.get("b1", 2, "/file.txt", "").await.unwrap();
        assert_eq!(v2.as_ref(), b"version 2");
    }
}
