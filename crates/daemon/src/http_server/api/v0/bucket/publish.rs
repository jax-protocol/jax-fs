use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use common::prelude::MountError;
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::http_server::api::client::ApiRequest;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize, clap::Args)]
pub struct PublishRequest {
    /// Bucket ID to publish
    #[arg(long)]
    pub bucket_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishResponse {
    pub bucket_id: Uuid,
    pub published: bool,
    pub new_bucket_link: String,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<PublishRequest>,
) -> Result<impl IntoResponse, PublishError> {
    tracing::info!("PUBLISH API: Publishing bucket {}", req.bucket_id);

    // Load mount at current head
    let mount = state.peer().mount(req.bucket_id).await?;

    // Check if already published
    if mount.is_published().await {
        tracing::info!("PUBLISH API: Bucket {} is already published", req.bucket_id);
        // Still return success, just note it's already published
    }

    // Save mount with publish=true (grants decryption to all mirrors)
    // This saves to blobs, appends to log, and notifies peers
    let new_bucket_link = state.peer().save_mount(&mount, true).await?;

    tracing::info!(
        "PUBLISH API: Bucket {} published, new link: {}",
        req.bucket_id,
        new_bucket_link.hash()
    );

    Ok((
        http::StatusCode::OK,
        Json(PublishResponse {
            bucket_id: req.bucket_id,
            published: true,
            new_bucket_link: new_bucket_link.hash().to_string(),
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    #[error("Mount error: {0}")]
    Mount(#[from] MountError),
}

impl IntoResponse for PublishError {
    fn into_response(self) -> Response {
        match self {
            PublishError::Mount(_) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Unexpected error".to_string(),
            )
                .into_response(),
        }
    }
}

// Client implementation - builds request for this operation
impl ApiRequest for PublishRequest {
    type Response = PublishResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket/publish").unwrap();
        client.post(full_url).json(&self)
    }
}
