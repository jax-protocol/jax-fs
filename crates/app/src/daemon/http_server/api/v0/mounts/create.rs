use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Json;
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::daemon::database::mount_queries::CreateMountParams;
use crate::daemon::http_server::api::client::ApiRequest;
use crate::ServiceState;

use super::MountInfoResponse;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMountRequest {
    /// Bucket ID to mount
    pub bucket_id: Uuid,
    /// Local mount point path
    pub mount_point: String,
    /// Auto-mount on daemon startup
    #[serde(default)]
    pub auto_mount: bool,
    /// Mount in read-only mode
    #[serde(default)]
    pub read_only: bool,
    /// Cache size in MB (default: 100)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_size_mb: Option<u32>,
    /// Cache TTL in seconds (default: 60)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_ttl_secs: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMountResponse {
    pub mount: MountInfoResponse,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<CreateMountRequest>,
) -> Result<impl IntoResponse, CreateMountError> {
    // Verify bucket exists
    let bucket = state
        .database()
        .get_bucket_info(&req.bucket_id)
        .await
        .map_err(|e| CreateMountError::Database(e.to_string()))?;

    if bucket.is_none() {
        return Err(CreateMountError::BucketNotFound(req.bucket_id));
    }

    // Check if mount point is already in use
    if let Some(_existing) = state
        .database()
        .get_mount_by_path(&req.mount_point)
        .await
        .map_err(|e| CreateMountError::Database(e.to_string()))?
    {
        return Err(CreateMountError::MountPointInUse(req.mount_point.clone()));
    }

    // Create the mount config
    let params = CreateMountParams {
        bucket_id: req.bucket_id,
        mount_point: req.mount_point,
        auto_mount: req.auto_mount,
        read_only: req.read_only,
        cache_size_mb: req.cache_size_mb,
        cache_ttl_secs: req.cache_ttl_secs,
    };

    let mount = state
        .database()
        .create_mount(params)
        .await
        .map_err(|e| CreateMountError::Database(e.to_string()))?;

    Ok((
        http::StatusCode::CREATED,
        Json(CreateMountResponse {
            mount: mount.into(),
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum CreateMountError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Bucket not found: {0}")]
    BucketNotFound(Uuid),
    #[error("Mount point already in use: {0}")]
    MountPointInUse(String),
}

impl IntoResponse for CreateMountError {
    fn into_response(self) -> Response {
        let status = match &self {
            CreateMountError::Database(_) => http::StatusCode::INTERNAL_SERVER_ERROR,
            CreateMountError::BucketNotFound(_) => http::StatusCode::NOT_FOUND,
            CreateMountError::MountPointInUse(_) => http::StatusCode::CONFLICT,
        };
        (status, Json(serde_json::json!({ "error": self.to_string() }))).into_response()
    }
}

impl ApiRequest for CreateMountRequest {
    type Response = CreateMountResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/mounts").unwrap();
        client.post(full_url).json(&self)
    }
}
