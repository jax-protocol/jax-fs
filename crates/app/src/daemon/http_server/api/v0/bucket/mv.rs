use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use common::prelude::{Link, MountError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MvRequest {
    /// Bucket ID containing the file/directory to move
    pub bucket_id: Uuid,
    /// Current absolute path of the file/directory
    pub source_path: String,
    /// New absolute path for the file/directory
    pub dest_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MvResponse {
    pub source_path: String,
    pub dest_path: String,
    pub link: Link,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<MvRequest>,
) -> Result<impl IntoResponse, MvError> {
    tracing::info!(
        "MV API: Moving {} to {} in bucket {}",
        req.source_path,
        req.dest_path,
        req.bucket_id
    );

    // Validate paths are absolute
    let source_path = PathBuf::from(&req.source_path);
    let dest_path = PathBuf::from(&req.dest_path);

    if !source_path.is_absolute() {
        return Err(MvError::InvalidPath(format!(
            "Source path must be absolute: {}",
            req.source_path
        )));
    }

    if !dest_path.is_absolute() {
        return Err(MvError::InvalidPath(format!(
            "Destination path must be absolute: {}",
            req.dest_path
        )));
    }

    // Load mount at current head
    let mut mount = state.peer().mount(req.bucket_id).await?;
    tracing::info!("MV API: Loaded mount for bucket {}", req.bucket_id);

    // Perform the move operation
    mount.mv(&source_path, &dest_path).await.map_err(|e| {
        tracing::error!("MV API: Failed to move: {}", e);
        MvError::Mount(e)
    })?;

    tracing::info!("MV API: Moved {} to {}", req.source_path, req.dest_path);

    // Save mount and update log
    let new_bucket_link = state.peer().save_mount(&mount).await?;

    tracing::info!(
        "MV API: Moved {} to {} in bucket {}, new link: {}",
        req.source_path,
        req.dest_path,
        req.bucket_id,
        new_bucket_link.hash()
    );

    Ok((
        http::StatusCode::OK,
        Json(MvResponse {
            source_path: req.source_path,
            dest_path: req.dest_path,
            link: new_bucket_link,
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum MvError {
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("Mount error: {0}")]
    Mount(#[from] MountError),
}

impl IntoResponse for MvError {
    fn into_response(self) -> Response {
        match self {
            MvError::InvalidPath(msg) => (
                http::StatusCode::BAD_REQUEST,
                format!("Invalid path: {}", msg),
            )
                .into_response(),
            MvError::Mount(MountError::PathNotFound(path)) => (
                http::StatusCode::NOT_FOUND,
                format!("Source not found: {}", path.display()),
            )
                .into_response(),
            MvError::Mount(MountError::PathAlreadyExists(path)) => (
                http::StatusCode::CONFLICT,
                format!("Destination already exists: {}", path.display()),
            )
                .into_response(),
            MvError::Mount(MountError::MoveIntoSelf { from, to }) => (
                http::StatusCode::BAD_REQUEST,
                format!(
                    "Cannot move '{}' into itself: destination '{}' is inside source",
                    from.display(),
                    to.display()
                ),
            )
                .into_response(),
            MvError::Mount(_) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Unexpected error".to_string(),
            )
                .into_response(),
        }
    }
}
