use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use common::prelude::MountError;
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use common::crypto::PublicKey;

use crate::daemon::http_server::api::client::ApiRequest;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize, clap::Args)]
pub struct ShareRequest {
    /// Bucket ID to share
    #[arg(long)]
    pub bucket_id: Uuid,

    /// Public key of the peer to share with (hex-encoded)
    #[arg(long)]
    pub peer_public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareResponse {
    pub bucket_id: Uuid,
    pub peer_public_key: String,
    pub new_bucket_link: String,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<ShareRequest>,
) -> Result<impl IntoResponse, ShareError> {
    tracing::info!(
        "SHARE API: Received share request for bucket {} with peer {}",
        req.bucket_id,
        req.peer_public_key
    );

    // Parse the peer's public key from hex
    let peer_public_key = PublicKey::from_hex(&req.peer_public_key)
        .map_err(|e| ShareError::InvalidPublicKey(e.to_string()))?;

    tracing::info!("SHARE API: Parsed peer public key successfully");

    // Load mount at current head
    let mut mount = state.peer().mount(req.bucket_id).await?;
    tracing::info!("SHARE API: Loaded mount for bucket {}", req.bucket_id);

    // Share bucket with peer
    mount.share(peer_public_key).await?;
    tracing::info!(
        "SHARE API: Mount.share() completed for peer {}",
        req.peer_public_key
    );

    tracing::info!("SHARE API: Calling save_mount for bucket {}", req.bucket_id);
    // Save mount and update log
    let new_bucket_link = state.peer().save_mount(&mount).await?;

    tracing::info!(
        "SHARE API: Bucket {} shared with peer {}, new link: {}",
        req.bucket_id,
        req.peer_public_key,
        new_bucket_link.hash()
    );

    Ok((
        http::StatusCode::OK,
        Json(ShareResponse {
            bucket_id: req.bucket_id,
            peer_public_key: req.peer_public_key,
            new_bucket_link: new_bucket_link.hash().to_string(),
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum ShareError {
    #[error("Invalid public key: {0}")]
    InvalidPublicKey(String),
    #[error("Mount error: {0}")]
    Mount(#[from] MountError),
}

impl IntoResponse for ShareError {
    fn into_response(self) -> Response {
        match self {
            ShareError::InvalidPublicKey(msg) => (
                http::StatusCode::BAD_REQUEST,
                format!("Invalid public key: {}", msg),
            )
                .into_response(),
            ShareError::Mount(_) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Unexpected error".to_string(),
            )
                .into_response(),
        }
    }
}

// Client implementation - builds request for this operation
impl ApiRequest for ShareRequest {
    type Response = ShareResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket/share").unwrap();
        client.post(full_url).json(&self)
    }
}
