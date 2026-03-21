pub mod actor;

use std::path::Path;

use async_trait::async_trait;
use axum::extract::FromRequestParts;
use bytes::Bytes;
use common::linked_data::Hash;
use common::prelude::Mime;
use http::request::Parts;
use object_store::Storage;
use uuid::Uuid;

use crate::database::models::GatewayCacheEntry;
use crate::database::Database;
use crate::ServiceState;

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

/// Gateway cache extracted from request parts.
///
/// Pulls `Database` from axum `State<ServiceState>` and `Storage` from
/// a layer extension. If the `Storage` extension is absent (cache not
/// configured), extraction yields `None` via `Option<GatewayCache>`.
#[derive(Clone)]
pub struct GatewayCache {
    db: Database,
    store: Storage,
}

#[async_trait]
impl FromRequestParts<ServiceState> for GatewayCache {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &ServiceState,
    ) -> Result<Self, Self::Rejection> {
        // Storage is always present — added as an extension layer in run_gateway
        let store = parts
            .extensions
            .get::<Storage>()
            .expect("Storage extension missing — gateway misconfigured");

        Ok(Self {
            db: state.database().clone(),
            store: store.clone(),
        })
    }
}

impl GatewayCache {
    /// Look up cached content. Returns (bytes, mime_type) on hit.
    pub async fn get(
        &self,
        bucket_id: &Uuid,
        height: u64,
        path: &Path,
        query_string: Option<&str>,
    ) -> Option<(Bytes, Mime)> {
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

        let link_hex = entry.link.to_string();

        match self.store.get_data(&link_hex).await {
            Ok(Some(data)) => Some((data, entry.mime_type)),
            Ok(None) => {
                tracing::debug!(
                    link = %link_hex,
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

    /// Populate the cache with content. The link is the content-addressed
    /// hash from the source blob — no rehashing.
    #[allow(clippy::too_many_arguments)]
    pub async fn put(
        &self,
        bucket_id: &Uuid,
        height: u64,
        path: &Path,
        query_string: Option<&str>,
        link: &Hash,
        data: &[u8],
        mime_type: &Mime,
    ) {
        if let Err(e) = self
            .store
            .put_data(&link.to_string(), Bytes::copy_from_slice(data))
            .await
        {
            tracing::warn!("cache put error (store): {}", e);
            return;
        }

        if let Err(e) = GatewayCacheEntry::log(
            bucket_id,
            height,
            path,
            query_string,
            link,
            data.len() as u64,
            mime_type,
            &self.db,
        )
        .await
        {
            tracing::warn!("cache put error (index): {}", e);
        }
    }
}

/// Spawn the background eviction actor. Call this when setting up the
/// gateway server, not from State.
pub fn spawn_eviction_actor(db: Database, store: Storage, config: CacheConfig) {
    let actor_instance = actor::CacheActor::new(db, store, config);
    tokio::spawn(actor_instance.run());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a GatewayCache directly for tests (no request parts needed).
    async fn test_cache() -> GatewayCache {
        let db = Database::memory().await.unwrap();
        let store = Storage::memory();
        GatewayCache { db, store }
    }

    #[tokio::test]
    async fn test_full_cache_flow() {
        let cache = test_cache().await;
        let bucket = Uuid::new_v4();
        let path = Path::new("/photo.jpg");

        // Miss
        assert!(cache.get(&bucket, 1, path, None).await.is_none());

        // Populate
        let data = b"fake jpeg data";
        let link = Hash::new(data);
        cache
            .put(&bucket, 1, path, None, &link, data, &mime::IMAGE_JPEG)
            .await;

        // Hit
        let (bytes, mime) = cache.get(&bucket, 1, path, None).await.unwrap();
        assert_eq!(bytes.as_ref(), data);
        assert_eq!(mime, mime::IMAGE_JPEG);
    }

    #[tokio::test]
    async fn test_query_string_cache_separately() {
        let cache = test_cache().await;
        let bucket = Uuid::new_v4();
        let path = Path::new("/photo.jpg");

        let orig = b"original";
        let thumb = b"thumbnail";
        cache
            .put(
                &bucket,
                1,
                path,
                None,
                &Hash::new(orig),
                orig,
                &mime::IMAGE_JPEG,
            )
            .await;
        cache
            .put(
                &bucket,
                1,
                path,
                Some("w=200"),
                &Hash::new(thumb),
                thumb,
                &mime::IMAGE_JPEG,
            )
            .await;

        let (original, _) = cache.get(&bucket, 1, path, None).await.unwrap();
        assert_eq!(original.as_ref(), b"original");

        let (thumbnail, _) = cache.get(&bucket, 1, path, Some("w=200")).await.unwrap();
        assert_eq!(thumbnail.as_ref(), b"thumbnail");
    }

    #[tokio::test]
    async fn test_content_dedup_across_paths() {
        let cache = test_cache().await;
        let bucket = Uuid::new_v4();
        let data = b"same content at different paths";
        let link = Hash::new(data);

        cache
            .put(
                &bucket,
                1,
                Path::new("/a.txt"),
                None,
                &link,
                data,
                &mime::TEXT_PLAIN,
            )
            .await;
        cache
            .put(
                &bucket,
                1,
                Path::new("/b.txt"),
                None,
                &link,
                data,
                &mime::TEXT_PLAIN,
            )
            .await;

        assert!(cache
            .get(&bucket, 1, Path::new("/a.txt"), None)
            .await
            .is_some());
        assert!(cache
            .get(&bucket, 1, Path::new("/b.txt"), None)
            .await
            .is_some());
    }

    #[tokio::test]
    async fn test_different_heights() {
        let cache = test_cache().await;
        let bucket = Uuid::new_v4();
        let path = Path::new("/file.txt");

        let v1 = b"version 1";
        let v2 = b"version 2";
        cache
            .put(
                &bucket,
                1,
                path,
                None,
                &Hash::new(v1),
                v1,
                &mime::TEXT_PLAIN,
            )
            .await;
        cache
            .put(
                &bucket,
                2,
                path,
                None,
                &Hash::new(v2),
                v2,
                &mime::TEXT_PLAIN,
            )
            .await;

        let (got_v1, _) = cache.get(&bucket, 1, path, None).await.unwrap();
        assert_eq!(got_v1.as_ref(), b"version 1");

        let (got_v2, _) = cache.get(&bucket, 2, path, None).await.unwrap();
        assert_eq!(got_v2.as_ref(), b"version 2");
    }
}
