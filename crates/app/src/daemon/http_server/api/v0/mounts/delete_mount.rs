use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::daemon::http_server::api::client::ApiRequest;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteMountResponse {
    pub deleted: bool,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Path(mount_id): Path<Uuid>,
) -> Result<impl IntoResponse, DeleteMountError> {
    // Check if mount exists and is not running
    let mount = state
        .database()
        .get_mount(&mount_id)
        .await
        .map_err(|e| DeleteMountError::Database(e.to_string()))?
        .ok_or(DeleteMountError::NotFound(mount_id))?;

    if mount.status == crate::daemon::database::mount_queries::MountStatus::Running {
        return Err(DeleteMountError::MountRunning(mount_id));
    }

    let deleted = state
        .database()
        .delete_mount(&mount_id)
        .await
        .map_err(|e| DeleteMountError::Database(e.to_string()))?;

    Ok((http::StatusCode::OK, Json(DeleteMountResponse { deleted })).into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum DeleteMountError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Mount not found: {0}")]
    NotFound(Uuid),
    #[error("Cannot delete running mount: {0}. Stop it first.")]
    MountRunning(Uuid),
}

impl IntoResponse for DeleteMountError {
    fn into_response(self) -> Response {
        let status = match &self {
            DeleteMountError::Database(_) => http::StatusCode::INTERNAL_SERVER_ERROR,
            DeleteMountError::NotFound(_) => http::StatusCode::NOT_FOUND,
            DeleteMountError::MountRunning(_) => http::StatusCode::CONFLICT,
        };
        (status, Json(serde_json::json!({ "error": self.to_string() }))).into_response()
    }
}

/// Request for client-side delete
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteMountRequest {
    pub mount_id: Uuid,
}

impl ApiRequest for DeleteMountRequest {
    type Response = DeleteMountResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url
            .join(&format!("/api/v0/mounts/{}", self.mount_id))
            .unwrap();
        client.delete(full_url)
    }
}
