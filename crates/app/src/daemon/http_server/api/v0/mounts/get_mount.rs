use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use uuid::Uuid;

use crate::ServiceState;

use super::MountInfoResponse;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GetMountResponse {
    pub mount: MountInfoResponse,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Path(mount_id): Path<Uuid>,
) -> Result<impl IntoResponse, GetMountError> {
    let mount = state
        .database()
        .get_mount(&mount_id)
        .await
        .map_err(|e| GetMountError::Database(e.to_string()))?
        .ok_or(GetMountError::NotFound(mount_id))?;

    Ok((
        http::StatusCode::OK,
        Json(GetMountResponse {
            mount: mount.into(),
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum GetMountError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Mount not found: {0}")]
    NotFound(Uuid),
}

impl IntoResponse for GetMountError {
    fn into_response(self) -> Response {
        let status = match &self {
            GetMountError::Database(_) => http::StatusCode::INTERNAL_SERVER_ERROR,
            GetMountError::NotFound(_) => http::StatusCode::NOT_FOUND,
        };
        (status, Json(serde_json::json!({ "error": self.to_string() }))).into_response()
    }
}
