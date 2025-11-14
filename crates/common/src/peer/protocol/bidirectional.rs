use std::fmt::Debug;

use anyhow::{anyhow, Result};
use iroh::endpoint::SendStream;
use iroh::protocol::AcceptError;
use iroh::Endpoint;
use serde::{Deserialize, Serialize};

use crate::bucket_log::BucketLogProvider;
use crate::crypto::PublicKey;
use crate::peer::Peer;

use super::{messages::Message, ALPN};

// TODO (amiller68): there should be a generic error type
//  for all the message / replies

/// Generic trait for handling bidirectional stream protocols
///
/// This trait eliminates boilerplate by providing default implementations
/// for all the serialization, stream I/O, and error handling logic.
///
/// Implementors only need to define the business logic:
/// - `handle_request`: What to send back when receiving a request (responder side)
/// - `handle_response`: What to do when receiving a response (initiator side)
pub trait BidirectionalHandler: Sized {
    /// The request message type
    type Message: Serialize + for<'de> Deserialize<'de> + Debug;

    /// The response message type
    type Reply: Serialize + for<'de> Deserialize<'de> + Debug;

    // TODO (amiller68): this is kinda stupid and brittle,
    //  but we basically need to wrap the request in the Message enum =
    //  we build by registering the request type with the register_handlers!
    //  macro
    // Each implementation must just implement the `wrap_request` method along the
    //  lines of:
    // fn wrap_request(request: Self::Request) -> Message {
    //     Message::Request(request)
    // }
    // for example Ping looks like this:
    // fn wrap_request(request: Self::Request) -> Message {
    //     Message::Ping(request)
    // }
    /// Wrap the request in the Message enum
    ///
    /// This must be used when sending requests to ensure the receiver can
    /// deserialize the correct enum variant.
    fn wrap_request(request: Self::Message) -> Message;

    /// Handle an incoming request and generate a response
    ///
    /// **Responder side:** Called when a request is received.
    ///
    /// Implement only the business logic - serialization and I/O are handled automatically.
    /// This is where you decide what to send back based on the request and peer state.
    fn handle_message<L: BucketLogProvider>(
        peer: &Peer<L>,
        sender_node_id: &PublicKey,
        message: &Self::Message,
    ) -> impl std::future::Future<Output = Self::Reply> + Send;

    /// Handle an incoming response and take action
    ///
    /// **Initiator side:** Called when a response is received.
    ///
    /// Implement only the business logic - deserialization is handled automatically.
    /// This is where you decide what to do based on the peer's response.
    fn handle_reply<L>(
        peer: &Peer<L>,
        recipient_node_id: &PublicKey,
        reply: &Self::Reply,
    ) -> impl std::future::Future<Output = Result<()>> + Send
    where
        L: BucketLogProvider,
        L::Error: std::error::Error + Send + Sync + 'static;

    /// Side effects after handling a request
    ///
    /// **Responder side:** Called after the response has been sent to the peer.
    ///
    /// This is where you can trigger background operations, spawn sync tasks,
    /// log metrics, etc. The response has already been sent, so this won't
    /// block the peer from receiving it.
    ///
    /// Default implementation does nothing.
    fn handle_message_side_effect<L>(
        _peer: &Peer<L>,
        _sender_node_id: &PublicKey,
        _message: &Self::Message,
        _reply: &Self::Reply,
    ) -> impl std::future::Future<Output = Result<()>> + Send
    where
        L: BucketLogProvider,
        L::Error: std::error::Error + Send + Sync + 'static,
    {
        async { Ok(()) }
    }

    /// Send a request to a peer and automatically handle the response
    async fn send<L>(
        peer: &Peer<L>,
        recipient_node_id: &PublicKey,
        request: Self::Message,
    ) -> Result<Self::Reply>
    where
        L: BucketLogProvider,
        L::Error: std::error::Error + Send + Sync + 'static,
    {
        let endpoint = peer.endpoint();
        let response = Self::_handle_send::<L>(endpoint, recipient_node_id, request).await?;
        Self::handle_reply(peer, recipient_node_id, &response).await?;
        Ok(response)
    }

    /// Handle an incoming request on the responder peer.
    ///  There is no need to call this directly, it's called
    ///  within the register_handler! macro.
    ///
    /// This is a provided method that handles all the boilerplate:
    /// - Calls the handler function
    /// - Serializes the response
    /// - Writes to the stream
    /// - Finishes the stream
    /// - Calls handle_response_side_effect hook for side effects
    /// - Error handling
    async fn _handle_message<L>(
        peer: &Peer<L>,
        sender_node_id: &PublicKey,
        message: Self::Message,
        mut send: SendStream,
    ) -> Result<(), AcceptError>
    where
        L: BucketLogProvider,
        L::Error: std::error::Error + Send + Sync + 'static,
    {
        // Call the handler to get the response
        let reply = Self::handle_message(peer, sender_node_id, &message).await;

        // Serialize the response
        let reply_bytes = bincode::serialize(&reply).map_err(|e| {
            tracing::error!("Failed to serialize reply: {}", e);
            let err: Box<dyn std::error::Error + Send + Sync> =
                anyhow!("failed to serialize reply: {}", e).into();
            AcceptError::from(err)
        })?;

        // Write the response to the stream
        send.write_all(&reply_bytes).await.map_err(|e| {
            tracing::error!("failed to send reply: {}", e);
            AcceptError::from(std::io::Error::other(e))
        })?;

        // Finish the stream
        send.finish().map_err(|e| {
            tracing::error!("failed to finish stream: {}", e);
            AcceptError::from(std::io::Error::other(e))
        })?;

        // Call the after_response_sent hook for side effects
        // This happens after the response is sent, so it won't block the peer
        if let Err(e) =
            Self::handle_message_side_effect(peer, sender_node_id, &message, &reply).await
        {
            tracing::error!("Error in after_response_sent hook: {}", e);
            // Don't fail the whole request if the side effect fails
        }

        Ok(())
    }

    /// Send a request to a peer and return the response
    ///  There is no need to call this directly, if you want
    ///  your client to handle the response as you define it within
    ///  your implementation, you should call the `send` method.
    ///
    /// This is a provided method that handles all the boilerplate:
    /// - Connects to the peer
    /// - Opens a bidirectional stream
    /// - Serializes and sends the request
    /// - Receives and deserializes the response
    /// - Returns the response for the caller to handle
    /// - Error handling
    ///
    /// If you want automatic response handling, call `handle_response` on the result.
    async fn _handle_send<L>(
        endpoint: &Endpoint,
        recipient_node_id: &PublicKey,
        message: Self::Message,
    ) -> Result<Self::Reply>
    where
        L: BucketLogProvider,
    {
        // Connect to the peer
        let conn = endpoint
            .connect(**recipient_node_id, ALPN)
            .await
            .map_err(|e| {
                tracing::error!("Failed to connect to peer {:?}: {}", recipient_node_id, e);
                anyhow!("Failed to connect to peer: {}", e)
            })?;

        tracing::info!("Connected to peer {:?}", recipient_node_id);

        // Open a bidirectional stream
        let (mut send, mut recv) = conn.open_bi().await.map_err(|e| {
            tracing::error!("Failed to open bidirectional stream: {}", e);
            anyhow!("Failed to open bidirectional stream: {}", e)
        })?;

        tracing::info!(
            "Opened bidirectional stream with peer {:?}",
            recipient_node_id
        );

        // Wrap request in Message enum and serialize
        let message = Self::wrap_request(message);
        let request_bytes = bincode::serialize(&message)
            .map_err(|e| anyhow!("Failed to serialize request: {}", e))?;

        tracing::info!(
            "BIDIRECTIONAL: Serialized request to {} bytes, first byte: {}",
            request_bytes.len(),
            request_bytes
                .get(0)
                .map(|b| b.to_string())
                .unwrap_or_else(|| "none".to_string())
        );

        // Send the request
        send.write_all(&request_bytes)
            .await
            .map_err(|e| anyhow!("Failed to write request: {}", e))?;

        tracing::info!("BIDIRECTIONAL: Sent request");

        send.finish()
            .map_err(|e| anyhow!("Failed to finish sending request: {}", e))?;

        tracing::info!("BIDIRECTIONAL: Finished sending request");

        // Read the response
        let response_bytes = recv
            .read_to_end(1024 * 1024)
            .await
            .map_err(|e| anyhow!("Failed to read response: {}", e))?;

        tracing::info!("BIDIRECTIONAL: Received response");

        // Deserialize the response
        let response: Self::Reply = bincode::deserialize(&response_bytes)
            .map_err(|e| anyhow!("Failed to deserialize response: {}", e))?;

        tracing::info!("BIDIRECTIONAL: Deserialized response: {:?}", response);

        Ok(response)
    }
}
