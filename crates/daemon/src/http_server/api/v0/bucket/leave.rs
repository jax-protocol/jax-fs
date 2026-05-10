use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::database::types::BucketAclEvent;
use crate::http_server::api::client::ApiRequest;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize, clap::Args)]
pub struct LeaveRequest {
    /// Bucket ID to leave
    #[arg(long)]
    pub bucket_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaveResponse {
    pub bucket_id: Uuid,
    pub status: String,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<LeaveRequest>,
) -> Result<impl IntoResponse, LeaveError> {
    tracing::info!("LEAVE BUCKET: {}", req.bucket_id);

    // Append left event
    state
        .database()
        .append_acl_event(&req.bucket_id, BucketAclEvent::Left, "self")
        .await
        .map_err(|e| LeaveError::Database(e.to_string()))?;

    // Stop any FUSE mounts for this bucket
    #[cfg(feature = "fuse")]
    {
        use crate::database::models::FuseMount;

        let mounts = FuseMount::by_bucket(req.bucket_id, state.database())
            .await
            .map_err(|e| LeaveError::Database(e.to_string()))?;

        if !mounts.is_empty() {
            let mount_manager = state.mount_manager().read().await;
            if let Some(mm) = mount_manager.as_ref() {
                for mount in &mounts {
                    if let Err(e) = mm.stop(&mount.mount_id).await {
                        tracing::warn!(
                            "Failed to stop mount {} for left bucket {}: {}",
                            mount.mount_id,
                            req.bucket_id,
                            e
                        );
                    }
                }
            }
        }
    }

    tracing::info!("LEAVE BUCKET: {} set to left", req.bucket_id);

    Ok((
        http::StatusCode::OK,
        Json(LeaveResponse {
            bucket_id: req.bucket_id,
            status: "left".to_string(),
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum LeaveError {
    #[error("Database error: {0}")]
    Database(String),
}

impl IntoResponse for LeaveError {
    fn into_response(self) -> Response {
        (http::StatusCode::INTERNAL_SERVER_ERROR, format!("{}", self)).into_response()
    }
}

impl ApiRequest for LeaveRequest {
    type Response = LeaveResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket/leave").unwrap();
        client.post(full_url).json(&self)
    }
}
