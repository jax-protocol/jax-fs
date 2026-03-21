use std::time::Duration;

use futures::StreamExt;
use uuid::Uuid;

use object_store::Storage;

use super::CacheConfig;
use crate::database::models::GatewayCacheEntry;
use crate::database::Database;

/// Hints the request path can send to the cache actor.
pub enum CacheHint {
    /// A bucket advanced to a new height — evict old entries for this bucket.
    BucketAdvanced { bucket_id: Uuid },
}

/// Background actor that owns eviction and cleanup.
///
/// The gateway writes to the cache inline (populate on miss) but never
/// blocks on cleanup — all eviction work happens here.
pub struct CacheActor {
    db: Database,
    store: Storage,
    config: CacheConfig,
    hints_rx: flume::Receiver<CacheHint>,
}

impl CacheActor {
    pub fn new(
        db: Database,
        store: Storage,
        config: CacheConfig,
        hints_rx: flume::Receiver<CacheHint>,
    ) -> Self {
        Self {
            db,
            store,
            config,
            hints_rx,
        }
    }

    /// Run the actor loop. Exits when the hints channel is closed.
    pub async fn run(self) {
        let Self {
            db,
            store,
            config,
            hints_rx,
        } = self;

        let mut interval =
            tokio::time::interval(Duration::from_secs(config.eviction_interval_secs));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let mut hint_stream = hints_rx.into_stream();

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    run_global_eviction(&db, &store, &config).await;
                }
                hint = hint_stream.next() => {
                    match hint {
                        Some(CacheHint::BucketAdvanced { bucket_id }) => {
                            evict_bucket(&bucket_id, &config, &db).await;
                        }
                        None => {
                            tracing::debug!("Cache actor shutting down — hints channel closed");
                            break;
                        }
                    }
                }
            }
        }
    }
}

/// Evict old heights for a single bucket that just advanced.
async fn evict_bucket(bucket_id: &Uuid, config: &CacheConfig, db: &Database) {
    match GatewayCacheEntry::evict_old_heights_for_bucket(bucket_id, config.max_versions, db).await
    {
        Ok(removed) if removed > 0 => {
            tracing::info!(%bucket_id, removed, "cache: evicted old heights for bucket");
        }
        Err(e) => {
            tracing::warn!(%bucket_id, "cache: failed to evict old heights for bucket: {}", e);
        }
        _ => {}
    }
}

/// Full eviction sweep across all buckets (timer-driven).
async fn run_global_eviction(db: &Database, store: &Storage, config: &CacheConfig) {
    // 1. Evict old height entries across all buckets
    match GatewayCacheEntry::evict_old_heights(config.max_versions, db).await {
        Ok(removed) if removed > 0 => {
            tracing::info!(removed, "cache: evicted old height entries");
        }
        Err(e) => {
            tracing::warn!("cache: failed to evict old heights: {}", e);
        }
        _ => {}
    }

    // 2. Evict expired entries
    if let Some(max_age) = config.max_entry_age_secs {
        match GatewayCacheEntry::evict_expired(max_age, db).await {
            Ok(hashes) if !hashes.is_empty() => {
                tracing::info!(count = hashes.len(), "cache: evicted expired entries");
            }
            Err(e) => {
                tracing::warn!("cache: failed to evict expired entries: {}", e);
            }
            _ => {}
        }
    }

    // 3. Enforce size limit via LRU eviction
    if let Some(max_size) = config.max_cache_size_bytes {
        match GatewayCacheEntry::evict_lru(max_size, db).await {
            Ok(hashes) if !hashes.is_empty() => {
                tracing::info!(
                    count = hashes.len(),
                    "cache: LRU-evicted entries for size limit"
                );
            }
            Err(e) => {
                tracing::warn!("cache: failed to LRU-evict: {}", e);
            }
            _ => {}
        }
    }

    // 4. Sweep unreferenced blobs from the content store
    sweep_unreferenced(db, store).await;
}

/// Remove blobs from the content store that are not referenced by any index entry.
async fn sweep_unreferenced(db: &Database, store: &Storage) {
    use futures::TryStreamExt;

    let referenced = match GatewayCacheEntry::referenced_links(db).await {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!("cache: failed to get referenced hashes: {}", e);
            return;
        }
    };

    let referenced_set: std::collections::HashSet<&str> =
        referenced.iter().map(|s| s.as_str()).collect();

    let stream = store.list_data_hashes_stream();
    let stored: Vec<String> = match std::pin::pin!(stream).try_collect().await {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!("cache: failed to list stored hashes: {}", e);
            return;
        }
    };

    let mut removed = 0u64;
    for hash in &stored {
        if !referenced_set.contains(hash.as_str()) {
            if let Err(e) = store.delete_data(hash).await {
                tracing::warn!(hash, "cache: failed to delete unreferenced blob: {}", e);
            } else {
                removed += 1;
            }
        }
    }

    if removed > 0 {
        tracing::info!(removed, "cache: swept unreferenced blobs");
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use bytes::Bytes;

    use super::*;

    struct TestEnv {
        db: Database,
        store: Storage,
        config: CacheConfig,
        bucket: Uuid,
    }

    async fn setup() -> TestEnv {
        TestEnv {
            db: Database::memory().await.unwrap(),
            store: Storage::memory(),
            config: CacheConfig {
                max_versions: 1,
                max_cache_size_bytes: Some(500),
                max_entry_age_secs: None,
                eviction_interval_secs: 86400,
            },
            bucket: Uuid::new_v4(),
        }
    }

    /// Helper: populate cache entries at multiple heights for a bucket.
    async fn populate_heights(bucket: &Uuid, heights: &[u64], db: &Database, store: &Storage) {
        for &h in heights {
            let data = format!("data-at-height-{}", h);
            let link = common::linked_data::Hash::new(data.as_bytes());
            store
                .put_data(&link.to_string(), Bytes::copy_from_slice(data.as_bytes()))
                .await
                .unwrap();
            GatewayCacheEntry::log(
                bucket,
                h,
                Path::new("/file.txt"),
                None,
                &link,
                data.len() as u64,
                &mime::TEXT_PLAIN,
                db,
            )
            .await
            .unwrap();
        }
    }

    #[tokio::test]
    async fn test_evict_bucket_keeps_latest_height() {
        let env = setup().await;

        populate_heights(&env.bucket, &[1, 2, 3], &env.db, &env.store).await;
        assert_eq!(GatewayCacheEntry::count(&env.db).await.unwrap(), 3);

        // Evict for this bucket — should keep only height 3
        evict_bucket(&env.bucket, &env.config, &env.db).await;
        assert_eq!(GatewayCacheEntry::count(&env.db).await.unwrap(), 1);

        assert!(
            GatewayCacheEntry::lookup(&env.bucket, 3, Path::new("/file.txt"), None, &env.db)
                .await
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn test_evict_bucket_does_not_touch_other_buckets() {
        let env = setup().await;
        let bob = Uuid::new_v4();

        populate_heights(&env.bucket, &[1, 2, 3], &env.db, &env.store).await;
        populate_heights(&bob, &[1, 2], &env.db, &env.store).await;
        assert_eq!(GatewayCacheEntry::count(&env.db).await.unwrap(), 5);

        // Evict only env.bucket — bob's entries should remain
        evict_bucket(&env.bucket, &env.config, &env.db).await;
        assert_eq!(GatewayCacheEntry::count(&env.db).await.unwrap(), 3); // 1 alice + 2 bob
    }

    #[tokio::test]
    async fn test_global_eviction_sweeps_all_buckets() {
        let env = setup().await;
        let bob = Uuid::new_v4();

        populate_heights(&env.bucket, &[1, 2, 3], &env.db, &env.store).await;
        populate_heights(&bob, &[1, 2], &env.db, &env.store).await;
        assert_eq!(GatewayCacheEntry::count(&env.db).await.unwrap(), 5);

        run_global_eviction(&env.db, &env.store, &env.config).await;

        // max_versions=1: env.bucket keeps height 3, bob keeps height 2
        assert_eq!(GatewayCacheEntry::count(&env.db).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_sweep_unreferenced_removes_orphan_blobs() {
        let env = setup().await;

        // Store a blob referenced by the index
        let referenced_data = b"referenced";
        let referenced_link = common::linked_data::Hash::new(referenced_data);
        env.store
            .put_data(
                &referenced_link.to_string(),
                Bytes::from_static(referenced_data),
            )
            .await
            .unwrap();
        GatewayCacheEntry::log(
            &env.bucket,
            1,
            Path::new("/kept.txt"),
            None,
            &referenced_link,
            referenced_data.len() as u64,
            &mime::TEXT_PLAIN,
            &env.db,
        )
        .await
        .unwrap();

        // Store an orphan blob (no index entry)
        env.store
            .put_data("orphan-hash", Bytes::from_static(b"orphaned"))
            .await
            .unwrap();

        // Before sweep: 2 blobs in store
        {
            use futures::TryStreamExt;
            let count: Vec<String> = std::pin::pin!(env.store.list_data_hashes_stream())
                .try_collect()
                .await
                .unwrap();
            assert_eq!(count.len(), 2);
        }

        sweep_unreferenced(&env.db, &env.store).await;

        // After sweep: orphan removed, referenced kept
        {
            use futures::TryStreamExt;
            let remaining: Vec<String> = std::pin::pin!(env.store.list_data_hashes_stream())
                .try_collect()
                .await
                .unwrap();
            assert_eq!(remaining.len(), 1);
            assert_eq!(remaining[0], referenced_link.to_string());
        }
    }

    #[tokio::test]
    async fn test_lru_eviction_respects_size_limit() {
        let env = setup().await;

        // Insert 3 entries of ~100 bytes each, total ~300
        for i in 0..3u64 {
            let data = format!("{:>100}", i); // 100 bytes each
            let link = common::linked_data::Hash::new(data.as_bytes());
            let path_str = format!("/file-{}.txt", i);
            env.store
                .put_data(&link.to_string(), Bytes::copy_from_slice(data.as_bytes()))
                .await
                .unwrap();
            GatewayCacheEntry::log(
                &env.bucket,
                1,
                Path::new(&path_str),
                None,
                &link,
                data.len() as u64,
                &mime::TEXT_PLAIN,
                &env.db,
            )
            .await
            .unwrap();
        }
        assert_eq!(GatewayCacheEntry::count(&env.db).await.unwrap(), 3);

        // Total is 300, limit 500 — no eviction
        run_global_eviction(&env.db, &env.store, &env.config).await;
        assert_eq!(GatewayCacheEntry::count(&env.db).await.unwrap(), 3);

        // Shrink limit to 150 — should evict LRU entries until under limit
        let tight_config = CacheConfig {
            max_versions: 100,
            max_cache_size_bytes: Some(150),
            ..CacheConfig::default()
        };
        run_global_eviction(&env.db, &env.store, &tight_config).await;
        assert!(GatewayCacheEntry::count(&env.db).await.unwrap() < 3);
    }
}
