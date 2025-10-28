use anyhow::anyhow;
use futures::future::BoxFuture;
use iroh::{
    endpoint::Connection,
    protocol::{AcceptError, ProtocolHandler},
};

mod messages;

use super::peer::Peer;
use messages::{Message, Reply};

// /// Callback type for handling announce messages
// pub type AnnounceCallback = Arc<
//     dyn Fn(uuid::Uuid, String, crate::linked_data::Link, Option<crate::linked_data::Link>)
//         + Send
//         + Sync,
// >;

// TODO ( amiller68): migrate the alpn, idt there's a great
//  reason to have an iroh prefix, nthis is not a n0 computer project
/// ALPN identifier for the JAX protocol
pub const ALPN: &[u8] = b"/iroh-jax/1";

// Implement the iroh protocol handler trait
// This allows the router to accept connections for this protocol
impl<BucketStateProvider: std::marker::Send + std::marker::Sync + std::fmt::Debug + 'static>
    ProtocolHandler for Peer<BucketStateProvider>
{
    #[allow(refining_impl_trait)]
    fn accept(
        &self,
        conn: iroh::endpoint::Connection,
    ) -> BoxFuture<'static, Result<(), AcceptError>> {
        Box::pin(async move {
            tracing::debug!("new connection from {:?}", conn.remote_node_id());
            let (mut send, mut recv) = conn.accept_bi().await.map_err(|e| {
                tracing::error!("failed to accept bidirectional stream: {}", e);
                AcceptError::from(e)
            })?;
            tracing::debug!("bidirectional stream accepted");

            // Get remote peer ID for announce handling
            let remote_node_id = conn.remote_node_id().map(|id| id.to_string());

            // NOTE (amiller68): our current request limit is 1MB,
            //  otherwise nodes will communicate over blobs for anything large.
            let message_bytes = recv.read_to_end(1024 * 1024).await.map_err(|e| {
                tracing::error!("failed to read message: {}", e);
                AcceptError::from(std::io::Error::other(e))
            })?; // 1MB limit

            let message: Message = bincode::deserialize(&message_bytes).map_err(|e| {
                tracing::error!("Failed to deserialize request: {}", e);
                let err: Box<dyn std::error::Error + Send + Sync> =
                    anyhow!("failed to deserialize message: {}", e).into();
                AcceptError::from(err)
            })?;

            match message {
                Message::Ping(ping_req) => {
                    tracing::info!(
                        "Received ping request for bucket {} with link {:?}",
                        ping_req.bucket_id,
                        ping_req.link
                    );
                    todo!()
                }
                _ => todo!(),
            }

            // // Dispatch based on request type
            // match request {
            //     Request::Ping(ping_req) => {
            //         tracing::info!(
            //             "Received ping request for bucket {} with link {:?}",
            //             ping_req.bucket_id,
            //             ping_req.current_link
            //         );

            //         // Check the bucket sync status using the state provider
            //         tracing::debug!("Checking bucket sync status");
            //         let status = self
            //             .state
            //             .check_bucket_sync(ping_req.bucket_id, &ping_req.current_link)
            //             .await
            //             .unwrap_or_else(|e| {
            //                 tracing::error!("Error checking bucket sync: {}", e);
            //                 super::messages::SyncStatus::NotFound
            //             });
            //         tracing::debug!("Bucket sync status: {:?}", status);

            //         let response = Response::Ping(PingResponse::new(status));
            //         tracing::debug!("Created ping response");

            //         // Serialize and send the response
            //         tracing::debug!("Serializing ping response");
            //         let response_bytes = bincode::serialize(&response).map_err(|e| {
            //             tracing::error!("Failed to serialize response: {}", e);
            //             let err: Box<dyn std::error::Error + Send + Sync> =
            //                 anyhow!("Failed to serialize response: {}", e).into();
            //             AcceptError::from(err)
            //         })?;
            //         tracing::debug!("Serialized response to {} bytes", response_bytes.len());

            //         tracing::debug!("Writing response to stream");
            //         send.write_all(&response_bytes).await.map_err(|e| {
            //             tracing::error!("Failed to write response: {}", e);
            //             AcceptError::from(std::io::Error::other(e))
            //         })?;

            //         tracing::debug!("Finishing send stream");
            //         send.finish().map_err(|e| {
            //             tracing::error!("Failed to finish send stream: {}", e);
            //             AcceptError::from(std::io::Error::other(e))
            //         })?;

            //         tracing::debug!("Waiting for connection to close");
            //         conn.closed().await;

            //         tracing::info!("Successfully sent ping response: {:?}", response);
            //     }

            //     Request::FetchBucket(fetch_req) => {
            //         tracing::debug!(
            //             "Received fetch bucket request for bucket {}",
            //             fetch_req.bucket_id
            //         );

            //         // Get the current bucket link
            //         let current_link = self
            //             .state
            //             .get_bucket_link(fetch_req.bucket_id)
            //             .await
            //             .unwrap_or_else(|e| {
            //                 tracing::error!("Error fetching bucket link: {}", e);
            //                 None
            //             });

            //         let response = Response::FetchBucket(FetchBucketResponse::new(current_link));

            //         // Serialize and send the response
            //         let response_bytes = bincode::serialize(&response).map_err(|e| {
            //             let err: Box<dyn std::error::Error + Send + Sync> =
            //                 anyhow!("Failed to serialize response: {}", e).into();
            //             AcceptError::from(err)
            //         })?;

            //         send.write_all(&response_bytes)
            //             .await
            //             .map_err(|e| AcceptError::from(std::io::Error::other(e)))?;

            //         send.finish()
            //             .map_err(|e| AcceptError::from(std::io::Error::other(e)))?;

            //         conn.closed().await;

            //         tracing::debug!("Sent fetch bucket response: {:?}", response);
            //     }

            //     Request::Announce(announce_msg) => {
            //         let peer_id_str = remote_node_id.unwrap_or_else(|_| "unknown".to_string());

            //         tracing::info!(
            //             "Received announce from peer {} for bucket {} with new link {:?}",
            //             peer_id_str,
            //             announce_msg.bucket_id,
            //             announce_msg.new_link
            //         );

            //         // Parse peer ID from the connection
            //         let peer_pub_key = match conn.remote_node_id() {
            //             Ok(node_id) => crate::crypto::PublicKey::from(node_id),
            //             Err(e) => {
            //                 tracing::error!("Failed to get remote node ID: {}", e);
            //                 send.finish()
            //                     .map_err(|e| AcceptError::from(std::io::Error::other(e)))?;
            //                 return Ok(());
            //             }
            //         };

            //         // Handle the announce directly using the sync logic
            //         let result = crate::peer::handle_announce(
            //             announce_msg.bucket_id,
            //             peer_pub_key,
            //             announce_msg.new_link,
            //             announce_msg.previous_link,
            //             self.state.clone(),
            //         )
            //         .await;

            //         if let Err(e) = result {
            //             tracing::error!(
            //                 "Failed to handle announce from peer {} for bucket {}: {}",
            //                 peer_id_str,
            //                 announce_msg.bucket_id,
            //                 e
            //             );
            //         }

            //         // No response needed for announce - just finish the stream
            //         send.finish()
            //             .map_err(|e| AcceptError::from(std::io::Error::other(e)))?;
            //     }
            // }

            Ok(())
        })
    }
}
