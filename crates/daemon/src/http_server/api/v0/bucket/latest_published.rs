use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use common::bucket_log::BucketLogProvider;

use crate::http_server::api::client::ApiRequest;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize, clap::Args)]
pub struct LatestPublishedRequest {
    /// The bucket ID to query
    #[arg(long)]
    pub bucket_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestPublishedResponse {
    pub bucket_id: Uuid,
    /// The link of the latest published version, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// The height of the latest published version, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u64>,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<LatestPublishedRequest>,
) -> Result<impl IntoResponse, LatestPublishedError> {
    tracing::info!(
        "LATEST_PUBLISHED: Querying latest published version for bucket {}",
        req.bucket_id
    );

    // Check if the bucket exists
    let exists = state
        .peer()
        .logs()
        .exists(req.bucket_id)
        .await
        .map_err(|e| LatestPublishedError::Internal(e.to_string()))?;

    if !exists {
        return Err(LatestPublishedError::BucketNotFound(req.bucket_id));
    }

    // Query latest published version
    let result = state
        .peer()
        .logs()
        .latest_published(req.bucket_id)
        .await
        .map_err(|e| LatestPublishedError::Internal(e.to_string()))?;

    let (link, height) = match result {
        Some((link, height)) => (Some(link.to_string()), Some(height)),
        None => (None, None),
    };

    tracing::info!(
        "LATEST_PUBLISHED: Bucket {} latest published: link={:?}, height={:?}",
        req.bucket_id,
        link,
        height
    );

    Ok((
        http::StatusCode::OK,
        Json(LatestPublishedResponse {
            bucket_id: req.bucket_id,
            link,
            height,
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum LatestPublishedError {
    #[error("Bucket not found: {0}")]
    BucketNotFound(Uuid),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for LatestPublishedError {
    fn into_response(self) -> Response {
        tracing::error!("LATEST_PUBLISHED ERROR: {:?}", self);
        match self {
            LatestPublishedError::BucketNotFound(id) => (
                http::StatusCode::NOT_FOUND,
                format!("Bucket not found: {}", id),
            )
                .into_response(),
            LatestPublishedError::Internal(msg) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Internal error: {}", msg),
            )
                .into_response(),
        }
    }
}

// Client implementation - builds request for this operation
impl ApiRequest for LatestPublishedRequest {
    type Response = LatestPublishedResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket/latest-published").unwrap();
        client.post(full_url).json(&self)
    }
}
