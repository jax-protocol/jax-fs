use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::daemon::http_server::api::client::ApiRequest;
use crate::ServiceState;

use super::MountInfoResponse;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopMountResponse {
    pub mount: MountInfoResponse,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Path(mount_id): Path<Uuid>,
) -> Result<impl IntoResponse, StopMountError> {
    // Verify mount exists
    let mount = state
        .database()
        .get_mount(&mount_id)
        .await
        .map_err(|e| StopMountError::Database(e.to_string()))?
        .ok_or(StopMountError::NotFound(mount_id))?;

    // Check if not running
    if mount.status != crate::daemon::database::mount_queries::MountStatus::Running {
        return Err(StopMountError::NotRunning(mount_id));
    }

    // Stop the mount via mount manager
    state
        .mount_manager()
        .stop_mount(&mount_id)
        .await
        .map_err(|e| StopMountError::StopFailed(e.to_string()))?;

    // Get updated mount info
    let updated_mount = state
        .database()
        .get_mount(&mount_id)
        .await
        .map_err(|e| StopMountError::Database(e.to_string()))?
        .ok_or(StopMountError::NotFound(mount_id))?;

    Ok((
        http::StatusCode::OK,
        Json(StopMountResponse {
            mount: updated_mount.into(),
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum StopMountError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Mount not found: {0}")]
    NotFound(Uuid),
    #[error("Mount not running: {0}")]
    NotRunning(Uuid),
    #[error("Failed to stop mount: {0}")]
    StopFailed(String),
}

impl IntoResponse for StopMountError {
    fn into_response(self) -> Response {
        let status = match &self {
            StopMountError::Database(_) => http::StatusCode::INTERNAL_SERVER_ERROR,
            StopMountError::NotFound(_) => http::StatusCode::NOT_FOUND,
            StopMountError::NotRunning(_) => http::StatusCode::CONFLICT,
            StopMountError::StopFailed(_) => http::StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(serde_json::json!({ "error": self.to_string() }))).into_response()
    }
}

/// Request for client-side stop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopMountRequest {
    pub mount_id: Uuid,
}

impl ApiRequest for StopMountRequest {
    type Response = StopMountResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url
            .join(&format!("/api/v0/mounts/{}/stop", self.mount_id))
            .unwrap();
        client.post(full_url)
    }
}
