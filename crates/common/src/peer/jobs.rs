//! Background job dispatcher for peer operations
//!
//! This module provides a lightweight job queue using flume channels for
//! coordinating background tasks like syncing, blob downloads, and other
//! potentially long-running operations.

use anyhow::Result;
use uuid::Uuid;

use crate::crypto::PublicKey;
use crate::linked_data::Link;

/// Background jobs that can be dispatched to the peer worker
#[derive(Debug, Clone)]
pub enum Job {
    /// Sync a bucket from a remote peer
    ///
    /// This job will download manifests, verify provenance, and update the local log.
    SyncBucket {
        /// The bucket to sync
        bucket_id: Uuid,
        /// The target link to sync to
        target_link: Link,
        /// The height at the target
        target_height: u64,
        /// The peer to sync from
        peer_id: PublicKey,
    },

    // TODO (amiller68): multipeer
    /// Download pins from a remote peer
    ///
    /// This job will download pins from a remote peer, verifying their provenance
    /// and updating the local pin store.
    DownloadPins {
        /// The bucket to download pins for
        pins_link: Link,
        /// The peers to download pins from
        peer_ids: Vec<PublicKey>,
    },

    /// Ping a peer to check bucket sync status
    ///
    /// This sends a ping to a peer for a specific bucket. The peer will respond
    /// with their bucket state, and if they're ahead, a sync job will be automatically
    /// dispatched.
    PingPeer {
        /// The bucket to ping about
        bucket_id: Uuid,
        /// The peer to ping
        peer_id: PublicKey,
    },
}

/// Job dispatcher that can be cloned and shared across tasks
///
/// This is a lightweight handle that can be cloned freely to send jobs
/// from anywhere in the application.
#[derive(Debug, Clone)]
pub struct JobDispatcher {
    tx: flume::Sender<Job>,
}

impl JobDispatcher {
    /// Create a new job dispatcher and receiver pair
    ///
    /// Returns a tuple of (dispatcher, receiver). The dispatcher can be cloned
    /// and shared, while the receiver should be given to the worker task.
    pub fn new() -> (Self, JobReceiver) {
        let (tx, rx) = flume::unbounded();
        (Self { tx }, JobReceiver { rx })
    }

    /// Dispatch a job to the background worker
    ///
    /// This is non-blocking and will succeed unless the receiver has been dropped.
    pub fn dispatch(&self, job: Job) -> Result<()> {
        tracing::info!("JOB_DISPATCHER: Dispatching job: {:?}", job);
        self.tx
            .send(job)
            .map_err(|_| anyhow::anyhow!("job receiver has been dropped"))
    }

    /// Dispatch a download pins job
    ///
    /// Convenience method for dispatching download pins jobs without constructing the Job enum manually.
    pub fn dispatch_download_pins(&self, pins_link: Link, peer_ids: Vec<PublicKey>) -> Result<()> {
        self.dispatch(Job::DownloadPins {
            pins_link,
            peer_ids,
        })
    }

    /// Dispatch a sync job
    ///
    /// Convenience method for dispatching sync jobs without constructing the Job enum manually.
    pub fn dispatch_sync(
        &self,
        bucket_id: Uuid,
        target_link: Link,
        target_height: u64,
        peer_id: PublicKey,
    ) -> Result<()> {
        self.dispatch(Job::SyncBucket {
            bucket_id,
            target_link,
            target_height,
            peer_id,
        })
    }
}

/// Job receiver for the background worker
///
/// This should be used by a dedicated worker task to process jobs.
#[derive(Debug)]
pub struct JobReceiver {
    rx: flume::Receiver<Job>,
}

impl JobReceiver {
    /// Receive the next job (blocking)
    ///
    /// Returns None when all senders have been dropped (graceful shutdown).
    pub fn recv(&self) -> Option<Job> {
        self.rx.recv().ok()
    }

    /// Try to receive a job without blocking
    ///
    /// Returns None if no jobs are available or all senders have been dropped.
    pub fn try_recv(&self) -> Option<Job> {
        self.rx.try_recv().ok()
    }

    /// Get an async receiver for use in async contexts
    ///
    /// This allows using the receiver with `.recv_async().await`.
    pub fn into_async(self) -> flume::r#async::RecvStream<'static, Job> {
        self.rx.into_stream()
    }
}
