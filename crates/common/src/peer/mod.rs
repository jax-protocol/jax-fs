use iroh::protocol::Router;
use tokio::sync::watch::Receiver as WatchReceiver;

mod blobs_store;
mod jobs;
mod peer;
mod peer_builder;
mod protocol;

pub use blobs_store::{BlobsStore, BlobsStoreError};
pub use jobs::{Job, JobDispatcher};
pub use protocol::ALPN;

pub use iroh::NodeAddr;

pub use peer::Peer;
pub use peer_builder::PeerBuilder;

/// Spawn the peer with protocol router and background job worker
///
/// This starts both the iroh protocol router (for handling incoming connections)
/// and the background job worker (for processing sync tasks and other jobs).
/// Both are gracefully shut down when the shutdown signal is received.
///
/// **Note:** This function can only be called once per peer instance, as it consumes
/// the internal job receiver. Subsequent calls will panic.
///
/// # Arguments
///
/// * `peer` - The peer instance to run (must be the original from PeerBuilder, not a clone)
/// * `shutdown_rx` - Watch receiver for shutdown signal
///
/// # Example
///
/// ```ignore
/// let peer = PeerBuilder::new()
///     .log_provider(database)
///     .build()
///     .await;
///
/// let (shutdown_tx, shutdown_rx) = watch::channel(());
///
/// tokio::spawn(async move {
///     if let Err(e) = peer::spawn(peer, shutdown_rx).await {
///         tracing::error!("Peer failed: {}", e);
///     }
/// });
/// ```
pub async fn spawn<L>(
    mut peer: Peer<L>,
    mut shutdown_rx: WatchReceiver<()>,
) -> Result<(), PeerError>
where
    L: crate::bucket_log::BucketLogProvider + Clone + Send + Sync + std::fmt::Debug + 'static,
    L::Error: std::fmt::Display + std::error::Error + Send + Sync + 'static,
{
    let node_id = peer.id();
    tracing::info!(peer_id = %node_id, "Starting peer");

    // Extract the job receiver (can only be done once)
    let job_receiver = peer.take_job_receiver().expect(
        "job receiver already consumed - peer::spawn can only be called once per peer instance",
    );

    // Extract what we need for the router before moving peer into worker
    let inner_blobs = peer.blobs().inner.clone();
    let endpoint = peer.endpoint().clone();
    let peer_for_router = peer.clone();

    // Spawn the background job worker (use the peer directly, no clone needed)
    let worker_handle = tokio::spawn(async move {
        tracing::info!(peer_id = %node_id, "Starting background job worker");
        peer.run_worker(job_receiver).await;
        tracing::info!(peer_id = %node_id, "Background job worker stopped normally");
    });

    // Build the protocol router with iroh-blobs and our custom protocol
    let router_builder = Router::builder(endpoint)
        .accept(iroh_blobs::ALPN, inner_blobs)
        .accept(ALPN, peer_for_router);

    let router = router_builder.spawn();

    tracing::info!(peer_id = %node_id, "Peer protocol router started");

    // Wait for shutdown signal
    let _ = shutdown_rx.changed().await;
    tracing::info!(peer_id = %node_id, "Shutdown signal received, stopping peer");

    // Shutdown the router (this closes the endpoint and stops accepting connections)
    router
        .shutdown()
        .await
        .map_err(|e| PeerError::RouterShutdown(e.into()))?;

    // Wait for the worker to finish (it will stop when the job dispatcher is dropped)
    // We give it a reasonable timeout to finish processing current jobs
    let worker_result =
        tokio::time::timeout(std::time::Duration::from_secs(30), worker_handle).await;

    match worker_result {
        Ok(Ok(())) => {
            tracing::info!(peer_id = %node_id, "Job worker stopped gracefully");
        }
        Ok(Err(e)) => {
            tracing::error!(peer_id = %node_id, error = %e, "Job worker panicked");
        }
        Err(_) => {
            tracing::warn!(peer_id = %node_id, "Job worker did not stop within timeout");
        }
    }

    tracing::info!(peer_id = %node_id, "Peer stopped");
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum PeerError {
    #[error("failed to shutdown router: {0}")]
    RouterShutdown(anyhow::Error),
}
