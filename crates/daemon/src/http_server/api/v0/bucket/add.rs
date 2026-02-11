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
pub struct FileUploadResult {
    pub mount_path: String,
    pub mime_type: String,
    pub size: usize,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddResponse {
    pub bucket_link: Link,
    pub files: Vec<FileUploadResult>,
    pub total_files: usize,
    pub successful_files: usize,
    pub failed_files: usize,
}

pub async fn handler(
    State(state): State<ServiceState>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AddError> {
    let mut bucket_id: Option<Uuid> = None;
    let mut base_path: Option<String> = None;
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();

    // Parse multipart form data
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::error!("Multipart parsing error: {}", e);
        AddError::MultipartError(e.to_string())
    })? {
        let field_name = field.name().unwrap_or("").to_string();

        match field_name.as_str() {
            "bucket_id" => {
                let text = field.text().await.map_err(|e| {
                    tracing::error!("Error reading bucket_id field: {}", e);
                    AddError::MultipartError(e.to_string())
                })?;
                bucket_id = Some(Uuid::parse_str(&text).map_err(|e| {
                    tracing::error!("Invalid bucket_id format: {}", e);
                    AddError::InvalidRequest("Invalid bucket_id".into())
                })?);
                tracing::info!("Parsed bucket_id: {}", bucket_id.unwrap());
            }
            "mount_path" => {
                base_path = Some(field.text().await.map_err(|e| {
                    tracing::error!("Error reading mount_path field: {}", e);
                    AddError::MultipartError(e.to_string())
                })?);
            }
            "file" | "files" => {
                // Get filename from the field
                let filename = field
                    .file_name()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unnamed".to_string());

                tracing::info!("Reading file: {}", filename);
                let file_data = field
                    .bytes()
                    .await
                    .map_err(|e| {
                        tracing::error!("Error reading file data for {}: {}", filename, e);
                        AddError::MultipartError(e.to_string())
                    })?
                    .to_vec();

                files.push((filename, file_data));
            }
            _ => {
                tracing::warn!("Ignoring unknown field: {}", field_name);
            }
        }
    }

    let bucket_id =
        bucket_id.ok_or_else(|| AddError::InvalidRequest("bucket_id is required".into()))?;
    let base_path =
        base_path.ok_or_else(|| AddError::InvalidRequest("mount_path is required".into()))?;

    if files.is_empty() {
        return Err(AddError::InvalidRequest(
            "At least one file is required".into(),
        ));
    }

    tracing::info!(
        "Uploading {} file(s) to bucket {} at path {}",
        files.len(),
        bucket_id,
        base_path
    );

    // Load mount at current head
    tracing::info!("Loading mount for bucket {}", bucket_id);
    let mut mount = state.peer().mount(bucket_id).await.map_err(|e| {
        tracing::error!("Failed to load mount for bucket {}: {}", bucket_id, e);
        e
    })?;

    let mut results = Vec::new();
    let mut successful = 0;
    let mut failed = 0;

    // Process each file
    tracing::info!("Processing {} files", files.len());
    for (idx, (filename, file_data)) in files.iter().enumerate() {
        tracing::info!("Processing file {}/{}: {}", idx + 1, files.len(), filename);

        // Construct full path
        let full_path = if base_path == "/" {
            format!("/{}", filename)
        } else {
            format!("{}/{}", base_path.trim_end_matches('/'), filename)
        };
        tracing::info!("Full path: {}", full_path);

        let mount_path_buf = PathBuf::from(&full_path);

        // Validate mount path
        if !mount_path_buf.is_absolute() {
            tracing::warn!("Path is not absolute: {}", full_path);
            results.push(FileUploadResult {
                mount_path: full_path.clone(),
                mime_type: String::new(),
                size: file_data.len(),
                success: false,
                error: Some("Mount path must be absolute".to_string()),
            });
            failed += 1;
            continue;
        }

        // Detect MIME type from file extension
        let mime_type = mime_guess::from_path(&mount_path_buf)
            .first_or_octet_stream()
            .to_string();

        let file_size = file_data.len();

        // Try to add file to mount
        match mount
            .add(&mount_path_buf, Cursor::new(file_data.clone()))
            .await
        {
            Ok(_) => {
                tracing::info!(
                    "✓ Added file {} ({} bytes, {})",
                    full_path,
                    file_size,
                    mime_type
                );
                results.push(FileUploadResult {
                    mount_path: full_path,
                    mime_type,
                    size: file_size,
                    success: true,
                    error: None,
                });
                successful += 1;
            }
            Err(e) => {
                tracing::error!("✗ Failed to add file {}: {}", full_path, e);
                results.push(FileUploadResult {
                    mount_path: full_path,
                    mime_type,
                    size: file_size,
                    success: false,
                    error: Some(e.to_string()),
                });
                failed += 1;
            }
        }
    }

    let bucket_link = if successful > 0 {
        tracing::info!("Saving mount (at least one file succeeded)");
        state.peer().save_mount(&mount, false).await.map_err(|e| {
            tracing::error!("Failed to save mount: {}", e);
            tracing::error!("Error details: {:?}", e);
            e
        })?
    } else {
        tracing::error!("All files failed to upload");
        return Err(AddError::InvalidRequest(
            "All files failed to upload".into(),
        ));
    };

    tracing::info!("Bucket link: {}", bucket_link);

    Ok((
        http::StatusCode::OK,
        axum::Json(AddResponse {
            bucket_link,
            files: results,
            total_files: successful + failed,
            successful_files: successful,
            failed_files: failed,
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum AddError {
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
            AddError::InvalidRequest(msg) | AddError::MultipartError(msg) => (
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
