use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::bucket_log::BucketLogProvider;
use crate::crypto::PublicKey;
use crate::linked_data::Link;
use crate::mount::Manifest;
use crate::peer::protocol::bidirectional::BidirectionalHandler;
use crate::peer::protocol::messages::Message;
use crate::peer::Peer;

/// Request to ping a peer and check bucket sync status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingMessage {
    /// The bucket ID to check
    pub bucket_id: Uuid,
    /// The current link the requesting peer has for this bucket
    pub link: Link,
    /// The height of the link we are responding to
    pub height: u64,
}

/// Sync status between two peers for a bucket
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PingReplyStatus {
    /// The peer does not have this bucket at all
    NotFound,
    /// We are ahead of the current peer's history,
    ///  report where we are
    Ahead(Link, u64),
    /// We are behind, report where we are
    Behind(Link, u64),
    /// Both agree on the current link (in sync)
    InSync,
}

/// Response to a ping request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingReply {
    /// The bucket ID being responded to
    pub bucket_id: Uuid,
    /// The sync status
    pub status: PingReplyStatus,
}

impl PingReply {
    /// Create a new pong message indicating bucket not found
    pub fn not_found(bucket_id: Uuid) -> Self {
        Self {
            bucket_id,
            status: PingReplyStatus::NotFound,
        }
    }

    /// Create a new pong message indicating we are ahead
    pub fn ahead(bucket_id: Uuid, link: Link, height: u64) -> Self {
        Self {
            bucket_id,
            status: PingReplyStatus::Ahead(link, height),
        }
    }

    /// Create a new pong message indicating we are behind
    pub fn behind(bucket_id: Uuid, link: Link, height: u64) -> Self {
        Self {
            bucket_id,
            status: PingReplyStatus::Behind(link, height),
        }
    }

    /// Create a new pong message indicating we are in sync
    pub fn in_sync(bucket_id: Uuid) -> Self {
        Self {
            bucket_id,
            status: PingReplyStatus::InSync,
        }
    }
}

/// Ping handler implementing the bidirectional handler pattern
///
/// This demonstrates the complete protocol flow in one place:
/// - Responder: what to send back + side effects after sending
/// - Initiator: what to do with the response
pub struct Ping;

impl BidirectionalHandler for Ping {
    type Message = PingMessage;
    type Reply = PingReply;

    /// Wrap the request in the Message enum for proper serialization
    fn wrap_request(request: Self::Message) -> Message {
        Message::Ping(request)
    }

    // ========================================
    // RESPONDER SIDE: When we receive a ping
    // ========================================

    /// Generate response: compare our state with peer's state
    async fn handle_message<L: BucketLogProvider>(
        peer: &Peer<L>,
        _sender_node_id: &PublicKey,
        ping: &PingMessage,
    ) -> PingReply {
        let logs = peer.logs();
        let bucket_id = ping.bucket_id;

        // Try to get our head for this bucket
        let (link, height) = match logs.head(bucket_id, None).await {
            Ok((link, height)) => (link, height),
            Err(_) => {
                // We don't have this bucket, return NotFound
                return PingReply::not_found(bucket_id);
            }
        };

        // Compare heights and determine sync status
        if height < ping.height {
            PingReply::behind(bucket_id, link, height)
        } else if height == ping.height {
            // At same height, we're in sync
            PingReply::in_sync(bucket_id)
        } else {
            // We're ahead of the remote peer
            PingReply::ahead(bucket_id, link, height)
        }
    }

    /// Side effects after sending response
    ///
    /// This is called AFTER we've sent the pong back to the peer.
    /// Use this to trigger background operations without blocking the response.
    async fn handle_message_side_effect<L: BucketLogProvider>(
        peer: &Peer<L>,
        sender_node_id: &PublicKey,
        ping: &PingMessage,
        pong: &PingReply,
    ) -> Result<()>
    where
        L::Error: std::error::Error + Send + Sync + 'static,
    {
        match &pong.status {
            PingReplyStatus::Behind(our_link, our_height) => {
                // We told them we're behind, so we should dispatch a sync job
                tracing::info!(
                    "We're behind peer for bucket {} (our height: {}, their height: {}), dispatching sync job",
                    ping.bucket_id,
                    our_height,
                    ping.height
                );

                // Load our manifest to get all peer IDs from shares
                let peer_ids = match peer.blobs().get_cbor::<Manifest>(&our_link.hash()).await {
                    Ok(manifest) => manifest.get_peer_ids(),
                    Err(e) => {
                        tracing::warn!(
                            "Failed to load manifest for peer list, using sender only: {}",
                            e
                        );
                        vec![*sender_node_id]
                    }
                };

                // Dispatch sync job to background worker
                use crate::peer::sync::{SyncBucketJob, SyncJob, SyncTarget};
                if let Err(e) = peer
                    .dispatch(SyncJob::SyncBucket(SyncBucketJob {
                        bucket_id: ping.bucket_id,
                        target: SyncTarget {
                            link: ping.link.clone(),
                            height: ping.height,
                            peer_ids,
                        },
                    }))
                    .await
                {
                    tracing::error!("Failed to dispatch sync job: {}", e);
                }
            }
            PingReplyStatus::Ahead(_, our_height) => {
                // We told them we're ahead, they might fetch from us
                tracing::debug!(
                    "We're ahead of peer for bucket {} (our height: {}, their height: {})",
                    ping.bucket_id,
                    our_height,
                    ping.height
                );
                // Nothing to do - they'll fetch from us if they want
            }
            PingReplyStatus::InSync => {
                tracing::debug!("In sync with peer for bucket {}", ping.bucket_id);
                // All good, nothing to do
            }
            PingReplyStatus::NotFound => {
                tracing::debug!(
                    "We don't have bucket {} that peer is asking about",
                    ping.bucket_id
                );
                // TODO (amiller68): there should probably be a share message instead
                //  of this
                // We don't have the bucket locally, so we can't get peer list from our manifest.
                // Use only the sender for now; once we sync we'll have the full peer list.
                let peer_ids = vec![*sender_node_id];

                // Dispatch sync job to background worker
                use crate::peer::sync::{SyncBucketJob, SyncJob, SyncTarget};
                if let Err(e) = peer
                    .dispatch(SyncJob::SyncBucket(SyncBucketJob {
                        bucket_id: ping.bucket_id,
                        target: SyncTarget {
                            link: ping.link.clone(),
                            height: ping.height,
                            peer_ids,
                        },
                    }))
                    .await
                {
                    tracing::error!("Failed to dispatch sync job: {}", e);
                }
            }
        }
        Ok(())
    }

    // ========================================
    // INITIATOR SIDE: When we receive a pong
    // ========================================

    /// Handle pong response: decide what to do based on sync status
    async fn handle_reply<L: BucketLogProvider>(
        peer: &Peer<L>,
        recipient_node_id: &PublicKey,
        pong: &PingReply,
    ) -> Result<()>
    where
        L::Error: std::error::Error + Send + Sync + 'static,
    {
        match &pong.status {
            PingReplyStatus::NotFound => {
                tracing::info!(
                    "Remote peer {} doesn't have bucket {}",
                    recipient_node_id.to_hex(),
                    pong.bucket_id
                );
                // The peer should attemp to fetch from us after this
            }
            PingReplyStatus::Ahead(link, height) => {
                // Remote peer is ahead, dispatch a sync job
                tracing::info!(
                    "Remote peer {} is ahead for bucket {} at height {} with link {:?}, dispatching sync job",
                    recipient_node_id.to_hex(),
                    pong.bucket_id,
                    height,
                    link
                );

                // Load our manifest to get all peer IDs from shares
                let peer_ids = match peer.logs().head(pong.bucket_id, None).await {
                    Ok((our_link, _)) => {
                        match peer.blobs().get_cbor::<Manifest>(&our_link.hash()).await {
                            Ok(manifest) => manifest.get_peer_ids(),
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to load manifest for peer list, using recipient only: {}",
                                    e
                                );
                                vec![*recipient_node_id]
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to get head for peer list, using recipient only: {}",
                            e
                        );
                        vec![*recipient_node_id]
                    }
                };

                // Dispatch sync job to background worker
                use crate::peer::sync::{SyncBucketJob, SyncJob, SyncTarget};
                if let Err(e) = peer
                    .dispatch(SyncJob::SyncBucket(SyncBucketJob {
                        bucket_id: pong.bucket_id,
                        target: SyncTarget {
                            link: link.clone(),
                            height: *height,
                            peer_ids,
                        },
                    }))
                    .await
                {
                    tracing::error!("Failed to dispatch sync job: {}", e);
                }
            }
            PingReplyStatus::Behind(link, height) => {
                tracing::info!(
                    "Remote peer {} is behind for bucket {} at height {} with link {:?}",
                    recipient_node_id.to_hex(),
                    pong.bucket_id,
                    height,
                    link
                );
                // Remote peer is behind, they might fetch from us
                // Nothing to do on our side
            }
            PingReplyStatus::InSync => {
                tracing::info!(
                    "In sync with peer {} for bucket {}",
                    recipient_node_id.to_hex(),
                    pong.bucket_id
                );
                // All good
            }
        }
        Ok(())
    }
}
