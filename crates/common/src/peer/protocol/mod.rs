use anyhow::anyhow;
use futures::future::BoxFuture;
use iroh::endpoint::Connection;
use iroh::protocol::{AcceptError, ProtocolHandler};

use crate::crypto::PublicKey;

use super::peer::Peer;

pub mod bidirectional;
pub mod messages;

use messages::Message;

// Re-export for external users implementing custom handlers
#[allow(unused_imports)]
pub use bidirectional::BidirectionalHandler;
pub use messages::ping::{Ping, PingMessage};

// TODO ( amiller68): migrate the alpn, idt there's a great
//  reason to have an iroh prefix, nthis is not a n0 computer project
/// ALPN identifier for the JAX protocol
pub const ALPN: &[u8] = b"/iroh-jax/1";

/// Generic connection handler that processes all incoming messages
///
/// This function handles all the boilerplate:
/// - Accepting bidirectional streams
/// - Reading and deserializing messages
/// - Dispatching to appropriate handlers
/// - Error handling
async fn handle_connection<L>(peer: Peer<L>, conn: Connection) -> Result<(), AcceptError>
where
    L: crate::bucket_log::BucketLogProvider,
    L::Error: std::error::Error + Send + Sync + 'static,
{
    // determine the sender
    let sender_node_id: PublicKey = conn
        .remote_node_id()
        .map_err(|e| {
            tracing::error!("failed to get remote node id: {}", e);
            AcceptError::from(e)
        })?
        .into();
    // Accept bidirectional stream
    let (send, mut recv) = conn.accept_bi().await.map_err(|e| {
        tracing::error!("failed to accept bidirectional stream: {}", e);
        AcceptError::from(e)
    })?;
    tracing::debug!("bidirectional stream accepted");

    // Read message (1MB limit for non-blob data)
    let message_bytes = recv.read_to_end(1024 * 1024).await.map_err(|e| {
        tracing::error!("failed to read message: {}", e);
        AcceptError::from(std::io::Error::other(e))
    })?;

    // Deserialize message
    let message: Message = bincode::deserialize(&message_bytes).map_err(|e| {
        tracing::error!("Failed to deserialize message: {}", e);
        tracing::error!(
            "First 20 bytes of received data: {:?}",
            &message_bytes[..message_bytes.len().min(20)]
        );
        let err: Box<dyn std::error::Error + Send + Sync> =
            anyhow!("failed to deserialize message: {}", e).into();
        AcceptError::from(err)
    })?;

    // Dispatch to appropriate handler
    message.dispatch(&peer, &sender_node_id, send).await?;

    Ok(())
}

// This allows the router to accept connections for this protocol
impl<L> ProtocolHandler for Peer<L>
where
    L: crate::bucket_log::BucketLogProvider,
    L::Error: std::error::Error + Send + Sync + 'static,
{
    #[allow(refining_impl_trait)]
    fn accept(&self, conn: Connection) -> BoxFuture<'static, Result<(), AcceptError>> {
        let peer = self.clone();
        Box::pin(handle_connection(peer, conn))
    }
}
