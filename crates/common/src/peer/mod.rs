use iroh::protocol::Router;
use tokio::sync::watch::Receiver as WatchReceiver;

mod blobs_store;
mod peer;
mod protocol;

pub use blobs_store::{BlobsStore, BlobsStoreError};
pub use protocol::ALPN;

// Re-export iroh types for convenience
pub use iroh::NodeAddr;

pub use crate::peer::peer::Peer;

pub async fn spawn<L>(
    peer: Peer<L>,
    mut shutdown_rx: WatchReceiver<()>,
) -> anyhow::Result<()>
where
    L: crate::bucket_log_provider::BucketLogProvider + Clone + Send + Sync + std::fmt::Debug + 'static,
    L::Error: std::fmt::Display,
{
    let inner_blobs = peer.blobs().inner.clone();
    let mut router_builder = Router::builder(peer.endpoint().clone())
        .accept(iroh_blobs::ALPN, inner_blobs)
        .accept(ALPN, peer);

    let router = router_builder.spawn();

    let _ = shutdown_rx.changed().await;

    router.shutdown().await?;
    Ok(())
}
