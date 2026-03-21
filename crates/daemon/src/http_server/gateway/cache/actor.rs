use std::time::Duration;

use object_store::Storage;

use super::CacheConfig;
use crate::database::models::GatewayCacheEntry;
use crate::database::Database;

/// Background actor that runs periodic eviction sweeps.
///
/// The gateway writes to the cache inline (populate on miss) but never
/// blocks on cleanup — all eviction work happens here on a timer.
pub struct CacheActor {
    db: Database,
    store: Storage,
    config: CacheConfig,
}

impl CacheActor {
    pub fn new(db: Database, store: Storage, config: CacheConfig) -> Self {
        Self { db, store, config }
    }

    /// Run the actor loop. Runs eviction on a timer until dropped.
    pub async fn run(self) {
        let mut interval =
            tokio::time::interval(Duration::from_secs(self.config.eviction_interval_secs));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;
            run_global_eviction(&self.db, &self.store, &self.config).await;
        }
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
    use common::linked_data::Hash;
    use uuid::Uuid;

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

    async fn populate_heights(bucket: &Uuid, heights: &[u64], db: &Database, store: &Storage) {
        for &h in heights {
            let data = format!("data-at-height-{}", h);
            let link = Hash::new(data.as_bytes());
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
    async fn test_global_eviction_keeps_latest_height() {
        let env = setup().await;

        populate_heights(&env.bucket, &[1, 2, 3], &env.db, &env.store).await;
        assert_eq!(GatewayCacheEntry::count(&env.db).await.unwrap(), 3);

        run_global_eviction(&env.db, &env.store, &env.config).await;
        assert_eq!(GatewayCacheEntry::count(&env.db).await.unwrap(), 1);

        assert!(
            GatewayCacheEntry::lookup(&env.bucket, 3, Path::new("/file.txt"), None, &env.db)
                .await
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn test_global_eviction_sweeps_all_buckets() {
        let env = setup().await;
        let bob = Uuid::new_v4();

        populate_heights(&env.bucket, &[1, 2, 3], &env.db, &env.store).await;
        populate_heights(&bob, &[1, 2], &env.db, &env.store).await;
        assert_eq!(GatewayCacheEntry::count(&env.db).await.unwrap(), 5);

        run_global_eviction(&env.db, &env.store, &env.config).await;
        assert_eq!(GatewayCacheEntry::count(&env.db).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_sweep_unreferenced_removes_orphan_blobs() {
        let env = setup().await;

        let referenced_data = b"referenced";
        let referenced_link = Hash::new(referenced_data);
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

        env.store
            .put_data("orphan-hash", Bytes::from_static(b"orphaned"))
            .await
            .unwrap();

        sweep_unreferenced(&env.db, &env.store).await;

        {
            use futures::TryStreamExt;
            let remaining: Vec<String> = std::pin::pin!(env.store.list_data_hashes_stream())
                .try_collect()
                .await
                .unwrap();
            assert_eq!(remaining.len(), 1);
        }
    }

    #[tokio::test]
    async fn test_lru_eviction_respects_size_limit() {
        let env = setup().await;

        for i in 0..3u64 {
            let data = format!("{:>100}", i);
            let link = Hash::new(data.as_bytes());
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

        // Total ~300, limit 500 — no eviction
        run_global_eviction(&env.db, &env.store, &env.config).await;
        assert_eq!(GatewayCacheEntry::count(&env.db).await.unwrap(), 3);

        // Shrink limit — should evict
        let tight = CacheConfig {
            max_versions: 100,
            max_cache_size_bytes: Some(150),
            ..CacheConfig::default()
        };
        run_global_eviction(&env.db, &env.store, &tight).await;
        assert!(GatewayCacheEntry::count(&env.db).await.unwrap() < 3);
    }
}
