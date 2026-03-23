use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::database::types::BucketStatus;
use crate::http_server::api::client::ApiRequest;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize, clap::Args)]
pub struct RemoveRequest {
    /// Bucket ID to remove
    #[arg(long)]
    pub bucket_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveResponse {
    pub bucket_id: Uuid,
    pub status: String,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<RemoveRequest>,
) -> Result<impl IntoResponse, RemoveError> {
    tracing::info!("REMOVE BUCKET: {}", req.bucket_id);

    // Set status to ignored (preserves bucket_log entries as audit trail)
    state
        .database()
        .set_bucket_status(&req.bucket_id, BucketStatus::Ignored, None)
        .await
        .map_err(|e| RemoveError::Database(e.to_string()))?;

    // Stop any FUSE mounts for this bucket
    #[cfg(feature = "fuse")]
    {
        use crate::database::models::FuseMount;

        let mounts = FuseMount::by_bucket(req.bucket_id, state.database())
            .await
            .map_err(|e| RemoveError::Database(e.to_string()))?;

        if !mounts.is_empty() {
            let mount_manager = state.mount_manager().read().await;
            if let Some(mm) = mount_manager.as_ref() {
                for mount in &mounts {
                    if let Err(e) = mm.stop(&mount.mount_id).await {
                        tracing::warn!(
                            "Failed to stop mount {} for removed bucket {}: {}",
                            mount.mount_id,
                            req.bucket_id,
                            e
                        );
                    }
                }
            }
        }
    }

    tracing::info!("REMOVE BUCKET: {} set to ignored", req.bucket_id);

    Ok((
        http::StatusCode::OK,
        Json(RemoveResponse {
            bucket_id: req.bucket_id,
            status: "ignored".to_string(),
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum RemoveError {
    #[error("Database error: {0}")]
    Database(String),
}

impl IntoResponse for RemoveError {
    fn into_response(self) -> Response {
        (http::StatusCode::INTERNAL_SERVER_ERROR, format!("{}", self)).into_response()
    }
}

impl ApiRequest for RemoveRequest {
    type Response = RemoveResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket/remove").unwrap();
        client.post(full_url).json(&self)
    }
}
