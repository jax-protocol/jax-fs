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
pub struct StartMountResponse {
    pub mount: MountInfoResponse,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Path(mount_id): Path<Uuid>,
) -> Result<impl IntoResponse, StartMountError> {
    // Verify mount exists
    let mount = state
        .database()
        .get_mount(&mount_id)
        .await
        .map_err(|e| StartMountError::Database(e.to_string()))?
        .ok_or(StartMountError::NotFound(mount_id))?;

    // Check if already running
    if mount.status == crate::daemon::database::mount_queries::MountStatus::Running {
        return Err(StartMountError::AlreadyRunning(mount_id));
    }

    // Start the mount via mount manager
    state
        .mount_manager()
        .start_mount(&mount_id)
        .await
        .map_err(|e| StartMountError::StartFailed(e.to_string()))?;

    // Get updated mount info
    let updated_mount = state
        .database()
        .get_mount(&mount_id)
        .await
        .map_err(|e| StartMountError::Database(e.to_string()))?
        .ok_or(StartMountError::NotFound(mount_id))?;

    Ok((
        http::StatusCode::OK,
        Json(StartMountResponse {
            mount: updated_mount.into(),
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum StartMountError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Mount not found: {0}")]
    NotFound(Uuid),
    #[error("Mount already running: {0}")]
    AlreadyRunning(Uuid),
    #[error("Failed to start mount: {0}")]
    StartFailed(String),
}

impl IntoResponse for StartMountError {
    fn into_response(self) -> Response {
        let status = match &self {
            StartMountError::Database(_) => http::StatusCode::INTERNAL_SERVER_ERROR,
            StartMountError::NotFound(_) => http::StatusCode::NOT_FOUND,
            StartMountError::AlreadyRunning(_) => http::StatusCode::CONFLICT,
            StartMountError::StartFailed(_) => http::StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(serde_json::json!({ "error": self.to_string() }))).into_response()
    }
}

/// Request for client-side start
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartMountRequest {
    pub mount_id: Uuid,
}

impl ApiRequest for StartMountRequest {
    type Response = StartMountResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url
            .join(&format!("/api/v0/mounts/{}/start", self.mount_id))
            .unwrap();
        client.post(full_url)
    }
}
