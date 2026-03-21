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

/// Thin handle for the gateway cache eviction actor.
///
/// Does not own database or store state — those are passed by the caller.
/// Only holds the channel for sending hints to the background eviction actor.
#[derive(Clone)]
pub struct GatewayCacheHandle {
    hints_tx: flume::Sender<CacheHint>,
}

impl GatewayCacheHandle {
    /// Spawn the background eviction actor and return a handle.
    pub fn spawn(db: Database, store: CacheStore, config: CacheConfig) -> Self {
        let (hints_tx, hints_rx) = flume::bounded(64);

        let actor = actor::CacheActor::new(db, store, config, hints_rx);
        tokio::spawn(actor.run());

        Self { hints_tx }
    }

    /// Send a non-blocking hint to the cache actor.
    pub fn hint(&self, hint: CacheHint) {
        let _ = self.hints_tx.try_send(hint);
    }
}

/// Look up cached content. Returns (bytes, mime_type) on hit.
pub async fn get(
    bucket_id: &str,
    height: i64,
    path: &str,
    query_string: &str,
    db: &Database,
    store: &CacheStore,
) -> Option<(Bytes, String)> {
    // Layer 1: path index lookup
    let entry = match GatewayCacheEntry::lookup(bucket_id, height, path, query_string, db).await {
        Ok(Some(entry)) => entry,
        Ok(None) => return None,
        Err(e) => {
            tracing::warn!("cache layer 1 lookup error: {}", e);
            return None;
        }
    };

    // Layer 2: content store lookup
    match store.get(&entry.content_hash).await {
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

/// Parameters for populating the cache.
pub struct PutParams<'a> {
    pub bucket_id: &'a str,
    pub height: i64,
    pub path: &'a str,
    pub query_string: &'a str,
    pub data: &'a [u8],
    pub mime_type: &'a str,
}

/// Populate the cache with content.
pub async fn put(params: &PutParams<'_>, db: &Database, store: &CacheStore) {
    let PutParams {
        bucket_id,
        height,
        path,
        query_string,
        data,
        mime_type,
    } = params;
    // Layer 2: store content (content-addressed, deduped automatically)
    let hash = match store.put(data).await {
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
            height: *height,
            path,
            query_string,
            content_hash: &hash,
            content_size: data.len() as i64,
            mime_type,
        },
        db,
    )
    .await
    {
        tracing::warn!("cache put error (index): {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_deps() -> (Database, CacheStore) {
        let id = uuid::Uuid::new_v4();
        let url =
            url::Url::parse(&format!("sqlite:file:test_{id}?mode=memory&cache=shared")).unwrap();
        let db = Database::connect(&url).await.unwrap();
        let store = CacheStore::new_memory();
        (db, store)
    }

    #[tokio::test]
    async fn test_full_cache_flow() {
        let (db, store) = test_deps().await;

        // Miss
        assert!(get("bucket-1", 1, "/photo.jpg", "", &db, &store)
            .await
            .is_none());

        // Populate
        let data = b"fake jpeg data";
        put(
            &PutParams {
                bucket_id: "bucket-1",
                height: 1,
                path: "/photo.jpg",
                query_string: "",
                data,
                mime_type: "image/jpeg",
            },
            &db,
            &store,
        )
        .await;

        // Hit
        let (bytes, mime) = get("bucket-1", 1, "/photo.jpg", "", &db, &store)
            .await
            .unwrap();
        assert_eq!(bytes.as_ref(), data);
        assert_eq!(mime, "image/jpeg");
    }

    #[tokio::test]
    async fn test_query_string_cache_separately() {
        let (db, store) = test_deps().await;

        put(
            &PutParams {
                bucket_id: "b1",
                height: 1,
                path: "/photo.jpg",
                query_string: "",
                data: b"original",
                mime_type: "image/jpeg",
            },
            &db,
            &store,
        )
        .await;
        put(
            &PutParams {
                bucket_id: "b1",
                height: 1,
                path: "/photo.jpg",
                query_string: "w=200",
                data: b"thumbnail",
                mime_type: "image/jpeg",
            },
            &db,
            &store,
        )
        .await;

        let (original, _) = get("b1", 1, "/photo.jpg", "", &db, &store).await.unwrap();
        assert_eq!(original.as_ref(), b"original");

        let (thumb, _) = get("b1", 1, "/photo.jpg", "w=200", &db, &store)
            .await
            .unwrap();
        assert_eq!(thumb.as_ref(), b"thumbnail");
    }

    #[tokio::test]
    async fn test_content_dedup_across_paths() {
        let (db, store) = test_deps().await;
        let data = b"same content at different paths";

        for path in ["/a.txt", "/b.txt"] {
            put(
                &PutParams {
                    bucket_id: "b1",
                    height: 1,
                    path,
                    query_string: "",
                    data,
                    mime_type: "text/plain",
                },
                &db,
                &store,
            )
            .await;
        }

        assert!(get("b1", 1, "/a.txt", "", &db, &store).await.is_some());
        assert!(get("b1", 1, "/b.txt", "", &db, &store).await.is_some());
    }

    #[tokio::test]
    async fn test_different_heights() {
        let (db, store) = test_deps().await;

        for (h, content) in [(1, "version 1"), (2, "version 2")] {
            put(
                &PutParams {
                    bucket_id: "b1",
                    height: h,
                    path: "/file.txt",
                    query_string: "",
                    data: content.as_bytes(),
                    mime_type: "text/plain",
                },
                &db,
                &store,
            )
            .await;
        }

        let (v1, _) = get("b1", 1, "/file.txt", "", &db, &store).await.unwrap();
        assert_eq!(v1.as_ref(), b"version 1");

        let (v2, _) = get("b1", 2, "/file.txt", "", &db, &store).await.unwrap();
        assert_eq!(v2.as_ref(), b"version 2");
    }
}
