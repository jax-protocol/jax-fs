use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use common::prelude::MountError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteRequest {
    /// Bucket ID containing the file to delete
    pub bucket_id: Uuid,
    /// Absolute path to the file or directory to delete
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteResponse {
    pub path: String,
    pub new_bucket_link: String,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<DeleteRequest>,
) -> Result<impl IntoResponse, DeleteError> {
    tracing::info!(
        "DELETE API: Received delete request for path {} in bucket {}",
        req.path,
        req.bucket_id
    );

    // Validate path is absolute
    let path = PathBuf::from(&req.path);
    if !path.is_absolute() {
        return Err(DeleteError::InvalidPath(format!(
            "Path must be absolute: {}",
            req.path
        )));
    }

    // Load mount at current head
    let mut mount = state.peer().mount(req.bucket_id).await?;
    tracing::info!("DELETE API: Loaded mount for bucket {}", req.bucket_id);

    // Check if path exists before attempting delete
    if mount.get(&path).await.is_err() {
        return Err(DeleteError::PathNotFound(req.path.clone()));
    }

    // Remove the file/directory
    mount.rm(&path).await.map_err(|e| {
        tracing::error!("DELETE API: Failed to remove {}: {}", req.path, e);
        DeleteError::Mount(e)
    })?;

    tracing::info!("DELETE API: Removed {} from mount", req.path);

    // Save mount and update log
    let new_bucket_link = state.peer().save_mount(&mount).await?;

    tracing::info!(
        "DELETE API: Deleted {} from bucket {}, new link: {}",
        req.path,
        req.bucket_id,
        new_bucket_link.hash()
    );

    Ok((
        http::StatusCode::OK,
        Json(DeleteResponse {
            path: req.path,
            new_bucket_link: new_bucket_link.hash().to_string(),
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum DeleteError {
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("Path not found: {0}")]
    PathNotFound(String),
    #[error("Mount error: {0}")]
    Mount(#[from] MountError),
}

impl IntoResponse for DeleteError {
    fn into_response(self) -> Response {
        match self {
            DeleteError::InvalidPath(msg) => (
                http::StatusCode::BAD_REQUEST,
                format!("Invalid path: {}", msg),
            )
                .into_response(),
            DeleteError::PathNotFound(msg) => (
                http::StatusCode::NOT_FOUND,
                format!("Path not found: {}", msg),
            )
                .into_response(),
            DeleteError::Mount(_) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Unexpected error".to_string(),
            )
                .into_response(),
        }
    }
}
