use std::time::Duration;

use futures::StreamExt;

use super::store::CacheStore;
use super::CacheConfig;
use crate::database::models::GatewayCacheEntry;
use crate::database::Database;

/// Hints the request path can send to the cache actor.
pub enum CacheHint {
    /// A bucket advanced to a new height — old entries may be evictable.
    BucketAdvanced { bucket_id: String, new_height: i64 },
}

/// Background actor that owns eviction and cleanup.
///
/// The gateway writes to the cache inline (populate on miss) but never
/// blocks on cleanup — all eviction work happens here.
pub struct CacheActor {
    db: Database,
    store: CacheStore,
    config: CacheConfig,
    hints_rx: flume::Receiver<CacheHint>,
}

impl CacheActor {
    pub fn new(
        db: Database,
        store: CacheStore,
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
                    run_eviction(&db, &store, &config).await;
                }
                hint = hint_stream.next() => {
                    match hint {
                        Some(CacheHint::BucketAdvanced { .. }) => {
                            run_eviction(&db, &store, &config).await;
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

async fn run_eviction(db: &Database, store: &CacheStore, config: &CacheConfig) {
    // 1. Evict old height entries
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
async fn sweep_unreferenced(db: &Database, store: &CacheStore) {
    let referenced = match GatewayCacheEntry::referenced_links(db).await {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!("cache: failed to get referenced hashes: {}", e);
            return;
        }
    };

    let stored = match store.list_hashes().await {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!("cache: failed to list stored hashes: {}", e);
            return;
        }
    };

    let referenced_set: std::collections::HashSet<&str> =
        referenced.iter().map(|s| s.as_str()).collect();

    let mut removed = 0u64;
    for hash in &stored {
        if !referenced_set.contains(hash.as_str()) {
            if let Err(e) = store.delete(hash).await {
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
