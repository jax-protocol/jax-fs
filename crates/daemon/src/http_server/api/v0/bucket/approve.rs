use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::database::types::BucketAclEvent;
use crate::http_server::api::client::ApiRequest;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize, clap::Args)]
pub struct ApproveRequest {
    /// Bucket ID to approve
    #[arg(long)]
    pub bucket_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveResponse {
    pub bucket_id: Uuid,
    pub status: String,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<ApproveRequest>,
) -> Result<impl IntoResponse, ApproveError> {
    tracing::info!("APPROVE BUCKET: {}", req.bucket_id);

    state
        .database()
        .append_acl_event(&req.bucket_id, BucketAclEvent::Approved, "self")
        .await
        .map_err(|e| ApproveError::Database(e.to_string()))?;

    tracing::info!("APPROVE BUCKET: {} approved", req.bucket_id);

    Ok((
        http::StatusCode::OK,
        Json(ApproveResponse {
            bucket_id: req.bucket_id,
            status: "active".to_string(),
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum ApproveError {
    #[error("Database error: {0}")]
    Database(String),
}

impl IntoResponse for ApproveError {
    fn into_response(self) -> Response {
        (http::StatusCode::INTERNAL_SERVER_ERROR, format!("{}", self)).into_response()
    }
}

impl ApiRequest for ApproveRequest {
    type Response = ApproveResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket/approve").unwrap();
        client.post(full_url).json(&self)
    }
}
