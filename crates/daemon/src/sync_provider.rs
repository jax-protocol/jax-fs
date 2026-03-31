//! Queue-based implementation of SyncProvider for the daemon
//!
//! This module provides the app-specific implementation of `SyncProvider` using
//! a flume channel-based job queue with a background worker.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;

use common::peer::{SyncJob, SyncProvider};

/// Configuration for the queued sync provider
#[derive(Debug, Clone)]
pub struct QueuedSyncConfig {
    /// Maximum number of queued jobs. None means unbounded.
    pub max_queue_size: Option<usize>,
}

impl Default for QueuedSyncConfig {
    fn default() -> Self {
        Self {
            // Default to 1000 pending jobs to prevent unbounded memory growth
            max_queue_size: Some(1000),
        }
    }
}

/// Queue-based implementation of SyncProvider
///
/// This implementation uses a flume channel to queue sync jobs and processes
/// them in a background worker task. This provides backpressure and prevents
/// blocking protocol handlers.
#[derive(Debug, Clone)]
pub struct QueuedSyncProvider {
    tx: flume::Sender<SyncJob>,
}

impl QueuedSyncProvider {
    /// Create a new queued sync provider
    ///
    /// Returns a tuple of (provider, receiver). The receiver should be passed to
    /// the worker task.
    pub fn new(config: QueuedSyncConfig) -> (Self, JobReceiver) {
        let (tx, rx) = match config.max_queue_size {
            Some(size) => {
                tracing::info!("Creating bounded job queue with size {}", size);
                flume::bounded(size)
            }
            None => {
                tracing::info!("Creating unbounded job queue");
                flume::unbounded()
            }
        };

        (Self { tx }, JobReceiver { rx })
    }
}

#[async_trait]
impl<L> SyncProvider<L> for QueuedSyncProvider
where
    L: common::bucket_log::BucketLogProvider + Clone + Send + Sync + 'static,
    L::Error: std::error::Error + Send + Sync + 'static,
{
    async fn execute(&self, _peer: &common::peer::Peer<L>, job: SyncJob) -> Result<()> {
        tracing::debug!("Queueing job for background execution: {:?}", job);
        self.tx.try_send(job).map_err(|e| match e {
            flume::TrySendError::Full(_) => {
                anyhow::anyhow!("job queue is full - worker may be overloaded")
            }
            flume::TrySendError::Disconnected(_) => {
                anyhow::anyhow!("job worker has been stopped")
            }
        })
    }
}

/// Job receiver for the background worker
///
/// This should be consumed by calling `into_async()` and processing the stream
/// in a worker task.
#[derive(Debug)]
pub struct JobReceiver {
    rx: flume::Receiver<SyncJob>,
}

impl JobReceiver {
    /// Convert to an async stream for use in tokio::select!
    pub fn into_async(self) -> flume::r#async::RecvStream<'static, SyncJob> {
        self.rx.into_stream()
    }
}

/// Maximum number of concurrent ping tasks
const MAX_CONCURRENT_PINGS: usize = 10;

/// Interval between periodic ping batches (5 minutes)
const PERIODIC_PING_INTERVAL_SECS: u64 = 300;

/// Run the background worker for queued sync jobs
///
/// This function processes jobs from the queue and also runs periodic ping scheduling.
/// It should be spawned in a background task.
///
/// # Example
///
/// ```ignore
/// let (sync_provider, job_receiver) = QueuedSyncProvider::new(config);
/// let peer = PeerBuilder::new()
///     .with_sync_provider(Arc::new(sync_provider))
///     .build()
///     .await;
///
/// tokio::spawn(async move {
///     run_worker(peer, job_receiver.into_async()).await;
/// });
/// ```
pub async fn run_worker<L>(
    peer: common::peer::Peer<L>,
    mut job_stream: flume::r#async::RecvStream<'static, SyncJob>,
) where
    L: common::bucket_log::BucketLogProvider + Clone + Send + Sync + 'static,
    L::Error: std::error::Error + Send + Sync + 'static,
{
    use common::peer::sync::{execute_job, ping_peer};
    use futures::StreamExt;
    use tokio::time::{interval, Duration};

    tracing::info!("Starting background job worker for peer {}", peer.id());

    // Create interval timer for periodic pings (every 5 minutes)
    let mut ping_interval = interval(Duration::from_secs(PERIODIC_PING_INTERVAL_SECS));
    ping_interval.tick().await; // Skip first immediate tick

    // Semaphore to cap concurrent ping tasks
    let ping_semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_PINGS));

    // Guard to skip periodic batch if previous is still running
    let pings_in_flight = Arc::new(AtomicBool::new(false));

    loop {
        tokio::select! {
            // Process incoming jobs from the queue
            Some(job) = job_stream.next() => {
                match job {
                    // Spawn ping jobs concurrently so they don't block sync/download
                    SyncJob::PingPeer(ping_job) => {
                        let peer = peer.clone();
                        let semaphore = ping_semaphore.clone();
                        tokio::spawn(async move {
                            let _permit = semaphore.acquire().await;
                            if let Err(e) = ping_peer::execute(&peer, ping_job).await {
                                tracing::error!("Ping job failed: {}", e);
                            }
                        });
                    }
                    // Execute sync and download jobs inline (serial)
                    job => {
                        if let Err(e) = execute_job(&peer, job).await {
                            tracing::error!("Job execution failed: {}", e);
                        }
                    }
                }
            }

            // Periodic ping scheduler
            _ = ping_interval.tick() => {
                if pings_in_flight.load(Ordering::Relaxed) {
                    tracing::debug!("Skipping periodic pings — previous batch still running");
                    continue;
                }

                tracing::info!("Running periodic ping scheduler");
                let peer = peer.clone();
                let flag = pings_in_flight.clone();
                flag.store(true, Ordering::Relaxed);
                tokio::spawn(async move {
                    schedule_periodic_pings(&peer).await;
                    flag.store(false, Ordering::Relaxed);
                });
            }

            // Stream closed (all senders dropped)
            else => {
                tracing::info!("Job queue closed, shutting down worker");
                break;
            }
        }
    }

    tracing::info!("Background job worker shutting down for peer {}", peer.id());
}

/// Schedule periodic pings to all peers in all buckets
///
/// This is app-specific scheduling logic - calls peer.ping_bucket_peers()
/// for each bucket on a timer.
async fn schedule_periodic_pings<L>(peer: &common::peer::Peer<L>)
where
    L: common::bucket_log::BucketLogProvider + Clone + Send + Sync + 'static,
    L::Error: std::error::Error + Send + Sync + 'static,
{
    // Get all buckets with ACL status, filter out terminal states
    let all_buckets = match peer.logs().list_buckets().await {
        Ok(buckets) => buckets,
        Err(e) => {
            tracing::error!("Failed to list buckets for periodic pings: {}", e);
            return;
        }
    };

    let bucket_ids: Vec<_> = all_buckets
        .into_iter()
        .filter(|(_, status)| !status.is_terminal())
        .map(|(id, _)| id)
        .collect();

    tracing::debug!("Scheduling periodic pings for {} buckets", bucket_ids.len());

    // For each bucket, ping all peers in shares
    for bucket_id in bucket_ids {
        if let Err(e) = peer.ping(bucket_id).await {
            tracing::warn!("Failed to ping peers for bucket {}: {}", bucket_id, e);
        }
    }
}
