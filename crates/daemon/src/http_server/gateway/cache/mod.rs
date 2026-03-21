pub mod actor;
pub mod store;

use bytes::Bytes;

use actor::CacheHint;
use store::CacheStore;

use crate::database::models::gateway_cache_entry::UpsertParams;
use crate::database::models::GatewayCacheEntry;
use crate::database::Database;

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
/// Uses the shared `Database` for layer 1 (path index) and a dedicated
/// `CacheStore` for layer 2 (content blobs). The background actor handles
/// eviction and cleanup.
#[derive(Clone)]
pub struct GatewayCache {
    db: Database,
    store: CacheStore,
    hints_tx: flume::Sender<CacheHint>,
}

impl GatewayCache {
    /// Initialize the cache and spawn the background eviction actor.
    pub fn spawn(db: Database, store: CacheStore, config: CacheConfig) -> Self {
        let (hints_tx, hints_rx) = flume::bounded(64);

        let actor = actor::CacheActor::new(db.clone(), store.clone(), config, hints_rx);
        tokio::spawn(actor.run());

        Self {
            db,
            store,
            hints_tx,
        }
    }

    /// Create a cache handle without the background actor (for tests).
    #[cfg(test)]
    fn new_without_actor(db: Database, store: CacheStore) -> Self {
        let (hints_tx, _) = flume::bounded(64);
        Self {
            db,
            store,
            hints_tx,
        }
    }

    /// Look up cached content. Returns (bytes, mime_type) on hit.
    pub async fn get(
        &self,
        bucket_id: &str,
        height: i64,
        path: &str,
        query_string: &str,
    ) -> Option<(Bytes, String)> {
        // Layer 1: path index lookup
        let entry = match GatewayCacheEntry::lookup(bucket_id, height, path, query_string, &self.db)
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
        query_string: &str,
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
        if let Err(e) = GatewayCacheEntry::upsert(
            &UpsertParams {
                bucket_id,
                height,
                path,
                query_string,
                content_hash: &hash,
                content_size: data.len() as i64,
                mime_type,
            },
            &self.db,
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_cache() -> GatewayCache {
        let id = uuid::Uuid::new_v4();
        let url =
            url::Url::parse(&format!("sqlite:file:test_{id}?mode=memory&cache=shared")).unwrap();
        let db = Database::connect(&url).await.unwrap();
        let store = CacheStore::new_memory();
        GatewayCache::new_without_actor(db, store)
    }

    #[tokio::test]
    async fn test_full_cache_flow() {
        let cache = test_cache().await;

        // Miss
        assert!(cache.get("bucket-1", 1, "/photo.jpg", "").await.is_none());

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
    async fn test_query_string_cache_separately() {
        let cache = test_cache().await;

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
        let cache = test_cache().await;
        let data = b"same content at different paths";

        cache.put("b1", 1, "/a.txt", "", data, "text/plain").await;
        cache.put("b1", 1, "/b.txt", "", data, "text/plain").await;

        assert!(cache.get("b1", 1, "/a.txt", "").await.is_some());
        assert!(cache.get("b1", 1, "/b.txt", "").await.is_some());
    }

    #[tokio::test]
    async fn test_different_heights() {
        let cache = test_cache().await;

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
