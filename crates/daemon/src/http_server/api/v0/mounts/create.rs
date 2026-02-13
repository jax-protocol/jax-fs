//! Create mount API endpoint

use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::database::models::FuseMount;
use crate::http_server::api::client::ApiRequest;
use crate::ServiceState;

/// Request to create a new mount configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMountRequest {
    pub bucket_id: Uuid,
    pub mount_point: String,
    #[serde(default)]
    pub auto_mount: bool,
    #[serde(default)]
    pub read_only: bool,
    pub cache_size_mb: Option<u32>,
    pub cache_ttl_secs: Option<u32>,
}

/// Response containing the created mount
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMountResponse {
    pub mount: MountInfo,
}

/// Information about a mount configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountInfo {
    pub mount_id: Uuid,
    pub bucket_id: Uuid,
    pub mount_point: String,
    pub enabled: bool,
    pub auto_mount: bool,
    pub read_only: bool,
    pub cache_size_mb: u32,
    pub cache_ttl_secs: u32,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<FuseMount> for MountInfo {
    fn from(m: FuseMount) -> Self {
        Self {
            mount_id: *m.mount_id,
            bucket_id: *m.bucket_id,
            mount_point: m.mount_point,
            enabled: *m.enabled,
            auto_mount: *m.auto_mount,
            read_only: *m.read_only,
            cache_size_mb: m.cache_size_mb as u32,
            cache_ttl_secs: m.cache_ttl_secs as u32,
            status: m.status.as_str().to_string(),
            error_message: m.error_message,
            created_at: m.created_at.to_string(),
            updated_at: m.updated_at.to_string(),
        }
    }
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<CreateMountRequest>,
) -> Result<impl IntoResponse, CreateMountError> {
    let mount_manager = state.mount_manager().read().await;
    let mount_manager = mount_manager
        .as_ref()
        .ok_or(CreateMountError::MountManagerUnavailable)?;

    let mount = mount_manager
        .create_mount(
            req.bucket_id,
            &req.mount_point,
            req.auto_mount,
            req.read_only,
            req.cache_size_mb,
            req.cache_ttl_secs,
        )
        .await?;

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
    #[error("Mount manager unavailable")]
    MountManagerUnavailable,
    #[error("Mount error: {0}")]
    Mount(#[from] crate::fuse::MountError),
}

impl IntoResponse for CreateMountError {
    fn into_response(self) -> Response {
        match self {
            CreateMountError::MountManagerUnavailable => (
                http::StatusCode::SERVICE_UNAVAILABLE,
                "Mount manager not available",
            )
                .into_response(),
            CreateMountError::Mount(e) => {
                (http::StatusCode::BAD_REQUEST, format!("Mount error: {}", e)).into_response()
            }
        }
    }
}

impl ApiRequest for CreateMountRequest {
    type Response = CreateMountResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/mounts/").unwrap();
        client.post(full_url).json(&self)
    }
}
