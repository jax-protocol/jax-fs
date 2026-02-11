use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use common::crypto::PublicKey;

use crate::http_server::api::client::ApiRequest;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize, clap::Args)]
pub struct PingRequest {
    /// Bucket ID to ping about
    #[arg(long)]
    pub bucket_id: Uuid,

    /// Public key of the peer to ping (hex-encoded)
    #[arg(long)]
    pub peer_public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingResponse {
    pub bucket_id: Uuid,
    pub peer_public_key: String,
    pub success: bool,
    pub message: String,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<PingRequest>,
) -> Result<impl IntoResponse, PingError> {
    tracing::info!(
        "PING API: Received ping request for bucket {} to peer {}",
        req.bucket_id,
        req.peer_public_key
    );

    // Parse the peer's public key from hex
    let peer_public_key = PublicKey::from_hex(&req.peer_public_key)
        .map_err(|e| PingError::InvalidPublicKey(e.to_string()))?;

    tracing::info!("PING API: Parsed peer public key successfully");

    // Dispatch ping job
    use common::peer::sync::{PingPeerJob, SyncJob};
    state
        .peer()
        .dispatch(SyncJob::PingPeer(PingPeerJob {
            bucket_id: req.bucket_id,
            peer_id: peer_public_key,
        }))
        .await
        .map_err(|e| PingError::Failed(e.to_string()))?;

    tracing::info!(
        "PING API: Dispatched PingPeer job for bucket {} to peer {}",
        req.bucket_id,
        req.peer_public_key
    );

    Ok((
        http::StatusCode::OK,
        Json(PingResponse {
            bucket_id: req.bucket_id,
            peer_public_key: req.peer_public_key,
            success: true,
            message: "Ping job dispatched".to_string(),
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum PingError {
    #[error("Invalid public key: {0}")]
    InvalidPublicKey(String),
    #[error("Failed to dispatch ping: {0}")]
    Failed(String),
}

impl IntoResponse for PingError {
    fn into_response(self) -> Response {
        match self {
            PingError::InvalidPublicKey(msg) => (
                http::StatusCode::BAD_REQUEST,
                format!("Invalid public key: {}", msg),
            )
                .into_response(),
            PingError::Failed(msg) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to ping: {}", msg),
            )
                .into_response(),
        }
    }
}

// Client implementation - builds request for this operation
impl ApiRequest for PingRequest {
    type Response = PingResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket/ping").unwrap();
        client.post(full_url).json(&self)
    }
}
