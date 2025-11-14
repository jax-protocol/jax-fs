use axum::extract::{Multipart, State};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::path::PathBuf;
use uuid::Uuid;

use common::prelude::{Link, MountError};

use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize, clap::Args)]
pub struct AddRequest {
    /// Bucket ID to add file to
    #[arg(long)]
    pub bucket_id: Uuid,

    /// Path in bucket where file should be mounted
    #[arg(long)]
    pub mount_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddResponse {
    pub mount_path: String,
    pub link: Link,
    pub mime_type: String,
}

pub async fn handler(
    State(state): State<ServiceState>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AddError> {
    let mut bucket_id: Option<Uuid> = None;
    let mut mount_path: Option<String> = None;
    let mut file_data: Option<Vec<u8>> = None;

    // Parse multipart form data
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AddError::MultipartError(e.to_string()))?
    {
        let field_name = field.name().unwrap_or("").to_string();

        match field_name.as_str() {
            "bucket_id" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| AddError::MultipartError(e.to_string()))?;
                bucket_id = Some(
                    Uuid::parse_str(&text)
                        .map_err(|_| AddError::InvalidRequest("Invalid bucket_id".into()))?,
                );
            }
            "mount_path" => {
                mount_path = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AddError::MultipartError(e.to_string()))?,
                );
            }
            "file" => {
                file_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| AddError::MultipartError(e.to_string()))?
                        .to_vec(),
                );
            }
            _ => {}
        }
    }

    let bucket_id =
        bucket_id.ok_or_else(|| AddError::InvalidRequest("bucket_id is required".into()))?;
    let mount_path =
        mount_path.ok_or_else(|| AddError::InvalidRequest("mount_path is required".into()))?;
    let file_data = file_data.ok_or_else(|| AddError::InvalidRequest("file is required".into()))?;

    // Validate mount path
    let mount_path_buf = PathBuf::from(&mount_path);
    if !mount_path_buf.is_absolute() {
        return Err(AddError::InvalidPath("Mount path must be absolute".into()));
    }

    // Detect MIME type from file extension
    let mime_type = mime_guess::from_path(&mount_path_buf)
        .first_or_octet_stream()
        .to_string();

    tracing::info!(
        "Adding file to bucket {} at {} ({})",
        bucket_id,
        mount_path,
        mime_type
    );

    // Detect MIME type from file extension
    let mime_type = mime_guess::from_path(&mount_path_buf)
        .first_or_octet_stream()
        .to_string();

    // Load mount at current head
    let mut mount = state.peer().mount(bucket_id).await?;

    // Add file to mount
    let reader = Cursor::new(file_data);
    mount.add(&mount_path_buf, reader).await?;

    // Save mount and update log
    let new_bucket_link = state.peer().save_mount(&mount).await?;

    Ok((
        http::StatusCode::OK,
        axum::Json(AddResponse {
            mount_path,
            link: new_bucket_link,
            mime_type,
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum AddError {
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Multipart error: {0}")]
    MultipartError(String),
    #[error("Mount error: {0}")]
    Mount(#[from] MountError),
}

impl IntoResponse for AddError {
    fn into_response(self) -> Response {
        match self {
            AddError::InvalidPath(msg)
            | AddError::InvalidRequest(msg)
            | AddError::MultipartError(msg) => (
                http::StatusCode::BAD_REQUEST,
                format!("Bad request: {}", msg),
            )
                .into_response(),
            AddError::Mount(_) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Unexpected error".to_string(),
            )
                .into_response(),
        }
    }
}
