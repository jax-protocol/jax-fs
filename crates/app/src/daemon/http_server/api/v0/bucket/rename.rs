use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use common::prelude::{Link, MountError};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::path::PathBuf;
use uuid::Uuid;

use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameRequest {
    /// Bucket ID containing the file to rename
    pub bucket_id: Uuid,
    /// Current absolute path of the file
    pub old_path: String,
    /// New absolute path for the file
    pub new_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameResponse {
    pub old_path: String,
    pub new_path: String,
    pub link: Link,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<RenameRequest>,
) -> Result<impl IntoResponse, RenameError> {
    tracing::info!(
        "RENAME API: Renaming {} to {} in bucket {}",
        req.old_path,
        req.new_path,
        req.bucket_id
    );

    // Validate paths are absolute
    let old_path = PathBuf::from(&req.old_path);
    let new_path = PathBuf::from(&req.new_path);

    if !old_path.is_absolute() {
        return Err(RenameError::InvalidPath(format!(
            "Old path must be absolute: {}",
            req.old_path
        )));
    }

    if !new_path.is_absolute() {
        return Err(RenameError::InvalidPath(format!(
            "New path must be absolute: {}",
            req.new_path
        )));
    }

    // Load mount at current head
    let mut mount = state.peer().mount(req.bucket_id).await?;
    tracing::info!("RENAME API: Loaded mount for bucket {}", req.bucket_id);

    // Check if source exists
    let _source_node = mount.get(&old_path).await.map_err(|_| {
        RenameError::SourceNotFound(format!("Source path not found: {}", req.old_path))
    })?;

    // Check if destination already exists
    if mount.get(&new_path).await.is_ok() {
        return Err(RenameError::DestinationExists(format!(
            "Destination path already exists: {}",
            req.new_path
        )));
    }

    // Read the file content
    let file_data = mount.cat(&old_path).await.map_err(|e| {
        tracing::error!("RENAME API: Failed to read file content: {}", e);
        RenameError::Mount(e)
    })?;

    tracing::info!(
        "RENAME API: Read {} bytes from {}",
        file_data.len(),
        req.old_path
    );

    // Remove from old path
    mount.rm(&old_path).await.map_err(|e| {
        tracing::error!("RENAME API: Failed to remove old path: {}", e);
        RenameError::Mount(e)
    })?;

    tracing::info!("RENAME API: Removed file from {}", req.old_path);

    // Add to new path
    let reader = Cursor::new(file_data);
    mount.add(&new_path, reader).await.map_err(|e| {
        tracing::error!("RENAME API: Failed to add to new path: {}", e);
        RenameError::Mount(e)
    })?;

    tracing::info!("RENAME API: Added file to {}", req.new_path);

    // Save mount and update log
    let new_bucket_link = state.peer().save_mount(&mount).await?;

    tracing::info!(
        "RENAME API: Renamed {} to {} in bucket {}, new link: {}",
        req.old_path,
        req.new_path,
        req.bucket_id,
        new_bucket_link.hash()
    );

    Ok((
        http::StatusCode::OK,
        Json(RenameResponse {
            old_path: req.old_path,
            new_path: req.new_path,
            link: new_bucket_link,
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum RenameError {
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("Source not found: {0}")]
    SourceNotFound(String),
    #[error("Destination exists: {0}")]
    DestinationExists(String),
    #[error("Mount error: {0}")]
    Mount(#[from] MountError),
}

impl IntoResponse for RenameError {
    fn into_response(self) -> Response {
        match self {
            RenameError::InvalidPath(msg) => (
                http::StatusCode::BAD_REQUEST,
                format!("Invalid path: {}", msg),
            )
                .into_response(),
            RenameError::SourceNotFound(msg) => (
                http::StatusCode::NOT_FOUND,
                format!("Source not found: {}", msg),
            )
                .into_response(),
            RenameError::DestinationExists(msg) => (
                http::StatusCode::CONFLICT,
                format!("Destination exists: {}", msg),
            )
                .into_response(),
            RenameError::Mount(_) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Unexpected error".to_string(),
            )
                .into_response(),
        }
    }
}
