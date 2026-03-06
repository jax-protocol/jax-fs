use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use common::prelude::MountError;
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use common::crypto::PublicKey;

use crate::http_server::api::client::ApiRequest;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize, clap::Args)]
pub struct UnshareRequest {
    /// Bucket ID to remove share from
    #[arg(long)]
    pub bucket_id: Uuid,

    /// Public key of the peer to remove (hex-encoded)
    #[arg(long)]
    pub peer_public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnshareResponse {
    pub bucket_id: Uuid,
    pub peer_public_key: String,
    pub new_bucket_link: String,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<UnshareRequest>,
) -> Result<impl IntoResponse, UnshareError> {
    tracing::info!(
        "UNSHARE API: Received unshare request for bucket {} removing peer {}",
        req.bucket_id,
        req.peer_public_key,
    );

    // Parse the peer's public key from hex
    let peer_public_key = PublicKey::from_hex(&req.peer_public_key)
        .map_err(|e| UnshareError::InvalidPublicKey(e.to_string()))?;

    // Load mount at current head
    let mount = state.peer().mount(req.bucket_id).await?;

    // Remove the share (verifies caller is owner)
    mount.remove_share(peer_public_key).await?;

    // Save mount and update log
    let new_bucket_link = state.peer().save_mount(&mount).await?;

    tracing::info!(
        "UNSHARE API: Peer {} removed from bucket {}, new link: {}",
        req.peer_public_key,
        req.bucket_id,
        new_bucket_link.hash()
    );

    Ok((
        http::StatusCode::OK,
        Json(UnshareResponse {
            bucket_id: req.bucket_id,
            peer_public_key: req.peer_public_key,
            new_bucket_link: new_bucket_link.hash().to_string(),
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum UnshareError {
    #[error("Invalid public key: {0}")]
    InvalidPublicKey(String),
    #[error("Mount error: {0}")]
    Mount(#[from] MountError),
}

impl IntoResponse for UnshareError {
    fn into_response(self) -> Response {
        match &self {
            UnshareError::InvalidPublicKey(msg) => (
                http::StatusCode::BAD_REQUEST,
                format!("Invalid public key: {}", msg),
            )
                .into_response(),
            UnshareError::Mount(MountError::Unauthorized) => (
                http::StatusCode::FORBIDDEN,
                "Unauthorized: only owners can remove shares".to_string(),
            )
                .into_response(),
            UnshareError::Mount(MountError::ShareNotFound) => {
                (http::StatusCode::NOT_FOUND, "Share not found".to_string()).into_response()
            }
            UnshareError::Mount(_) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Unexpected error".to_string(),
            )
                .into_response(),
        }
    }
}

// Client implementation - builds request for this operation
impl ApiRequest for UnshareRequest {
    type Response = UnshareResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket/unshare").unwrap();
        client.post(full_url).json(&self)
    }
}
