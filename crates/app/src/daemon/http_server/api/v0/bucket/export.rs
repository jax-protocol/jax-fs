use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use common::bucket_log::BucketLogProvider;
use common::mount::{MountError, NodeLink};
use common::prelude::Mount;

use crate::ops::bucket::clone_state::PathHashMap;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportRequest {
    pub bucket_id: Uuid,
    pub target_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResponse {
    pub bucket_name: String,
    pub link: common::linked_data::Link,
    pub height: u64,
    pub files_exported: usize,
    pub hash_map: PathHashMap,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<ExportRequest>,
) -> Result<impl IntoResponse, ExportError> {
    tracing::info!(
        "EXPORT: Exporting bucket {} to {}",
        req.bucket_id,
        req.target_dir.display()
    );

    // Load the bucket from logs
    let logs = state.peer().logs();

    // Check if bucket exists first
    let exists = logs
        .exists(req.bucket_id)
        .await
        .map_err(|e| ExportError::BucketLog(e.to_string()))?;

    if !exists {
        return Err(ExportError::BucketNotFound(req.bucket_id));
    }

    let (head_link, height) = logs
        .head(req.bucket_id, None)
        .await
        .map_err(|e| ExportError::BucketLog(e.to_string()))?;

    let blobs = state.node().blobs();
    let secret_key = state.node().secret();

    // Load the mount
    let mount = Mount::load(&head_link, secret_key, blobs)
        .await
        .map_err(ExportError::Mount)?;

    // Get the bucket name from the mount's manifest
    let mount_inner = mount.inner().await;
    let bucket_name = mount_inner.manifest().name().to_string();

    // Create target directory if it doesn't exist
    std::fs::create_dir_all(&req.target_dir)?;

    // Export all files from the mount to the filesystem
    let mut hash_map = PathHashMap::new();
    let files_exported =
        export_mount_to_filesystem(&mount, &req.target_dir, blobs, &mut hash_map).await?;

    tracing::info!(
        "EXPORT: Successfully exported {} files from bucket {}",
        files_exported,
        req.bucket_id
    );

    Ok((
        http::StatusCode::OK,
        Json(ExportResponse {
            bucket_name,
            link: head_link,
            height,
            files_exported,
            hash_map,
        }),
    )
        .into_response())
}

/// Export the entire mount to a filesystem directory
async fn export_mount_to_filesystem(
    mount: &Mount,
    target_dir: &Path,
    blobs: &common::peer::BlobsStore,
    hash_map: &mut PathHashMap,
) -> Result<usize, ExportError> {
    let mut files_exported = 0;

    // Get all items recursively
    let items = mount
        .ls_deep(&PathBuf::from("/"))
        .await
        .map_err(ExportError::Mount)?;

    for (path, node_link) in items {
        match node_link {
            NodeLink::Data(link, secret, _) => {
                // This is a file - export it
                let target_path = target_dir.join(&path);

                // Create parent directories if needed
                if let Some(parent) = target_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                // Get encrypted blob
                let encrypted_data = blobs
                    .get(&link.hash())
                    .await
                    .map_err(|e| ExportError::BlobStore(e.to_string()))?;

                // Extract plaintext hash without full decryption (for hash map)
                let plaintext_hash = secret
                    .extract_plaintext_hash(&encrypted_data)
                    .map_err(|e| ExportError::Decryption(e.to_string()))?;

                // Decrypt and write file
                let decrypted_data = secret
                    .decrypt(&encrypted_data)
                    .map_err(|e| ExportError::Decryption(e.to_string()))?;

                std::fs::write(&target_path, decrypted_data)?;

                // Store mapping: path -> (blob_hash, plaintext_hash)
                hash_map.insert(path.clone(), link.hash(), plaintext_hash);

                files_exported += 1;

                tracing::debug!("EXPORT: Exported file {}", path.display());
            }
            NodeLink::Dir(_, _) => {
                // This is a directory - create it
                let target_path = target_dir.join(&path);
                std::fs::create_dir_all(&target_path)?;

                tracing::debug!("EXPORT: Created directory {}", path.display());
            }
        }
    }

    Ok(files_exported)
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("Bucket not found: {0}")]
    BucketNotFound(Uuid),
    #[error("Bucket log error: {0}")]
    BucketLog(String),
    #[error("Mount error: {0}")]
    Mount(#[from] MountError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Blob store error: {0}")]
    BlobStore(String),
    #[error("Decryption error: {0}")]
    Decryption(String),
}

impl IntoResponse for ExportError {
    fn into_response(self) -> Response {
        tracing::error!("EXPORT ERROR: {:?}", self);
        match self {
            ExportError::BucketNotFound(id) => (
                http::StatusCode::NOT_FOUND,
                format!("Bucket not found: {}", id),
            )
                .into_response(),
            ExportError::BucketLog(msg) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Bucket log error: {}", msg),
            )
                .into_response(),
            ExportError::Mount(e) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Mount error: {}", e),
            )
                .into_response(),
            ExportError::Io(e) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("I/O error: {}", e),
            )
                .into_response(),
            ExportError::BlobStore(msg) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Blob store error: {}", msg),
            )
                .into_response(),
            ExportError::Decryption(msg) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Decryption error: {}", msg),
            )
                .into_response(),
        }
    }
}
