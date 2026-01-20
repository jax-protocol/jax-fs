//! Sync provider abstraction and job execution
//!
//! This module defines the sync provider trait and job types. Each job type
//! has its own module with execution logic that can be reused by peers.

use anyhow::Result;
use async_trait::async_trait;
use thiserror::Error;

use crate::bucket_log::BucketLogProvider;
use crate::linked_data::Link;

/// Errors that can occur during bucket synchronization.
#[derive(Debug, Error)]
pub enum SyncError {
    #[error("not authorized: our key not in bucket shares")]
    NotAuthorized,
    #[error("invalid signature on manifest")]
    InvalidSignature,
    #[error("author not in manifest shares")]
    AuthorNotInShares,
    #[error("invalid manifest in chain at {link}: {reason}")]
    InvalidManifestInChain { link: Link, reason: String },
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

pub mod download_pins;
pub mod ping_peer;
pub mod sync_bucket;

// Re-export job types, helpers, and errors
pub use download_pins::DownloadPinsJob;
pub use ping_peer::PingPeerJob;
pub use sync_bucket::{SyncBucketJob, SyncTarget};

/// Background sync job types
///
/// These represent the different kinds of background work that can be dispatched.
#[derive(Debug, Clone)]
pub enum SyncJob {
    /// Sync a bucket from a remote peer
    SyncBucket(SyncBucketJob),
    /// Download pins from remote peers
    DownloadPins(DownloadPinsJob),
    /// Ping a peer to check bucket sync status
    PingPeer(PingPeerJob),
}

/// Execute a sync job by calling the appropriate module's execute function
///
/// This is a helper function that dispatches to the per-job-type execution logic.
/// Both synchronous and queued providers can use this.
pub async fn execute_job<L>(peer: &crate::peer::Peer<L>, job: SyncJob) -> Result<()>
where
    L: BucketLogProvider + Clone + Send + Sync + 'static,
    L::Error: std::error::Error + Send + Sync + 'static,
{
    match job {
        SyncJob::DownloadPins(job) => download_pins::execute(peer, job).await,
        SyncJob::SyncBucket(job) => sync_bucket::execute(peer, job).await,
        SyncJob::PingPeer(job) => ping_peer::execute(peer, job).await,
    }
}

/// Trait for sync provider implementations
///
/// This trait abstracts WHEN and WHERE sync jobs are executed. The actual sync
/// logic lives in the per-job modules. Implementations decide the execution
/// context:
///
/// - **Synchronous**: Execute immediately by calling execute_job directly
/// - **Queued**: Send to a channel for background worker processing
/// - **Actor-based**: Send to an actor mailbox for processing
///
/// This allows minimal peers to use simple synchronous execution, while complex
/// applications can decouple sync jobs from protocol handlers using queues.
#[async_trait]
pub trait SyncProvider<L>: Send + Sync + std::fmt::Debug
where
    L: BucketLogProvider + Clone + Send + Sync + 'static,
    L::Error: std::error::Error + Send + Sync + 'static,
{
    /// Execute a sync job with the given peer
    ///
    /// Implementations decide when and where this runs. The job execution logic
    /// is provided by the execute_job helper and per-job modules.
    async fn execute(&self, peer: &crate::peer::Peer<L>, job: SyncJob) -> Result<()>;
}
