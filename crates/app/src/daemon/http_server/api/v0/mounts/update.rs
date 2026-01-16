use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::daemon::database::mount_queries::UpdateMountParams;
use crate::daemon::http_server::api::client::ApiRequest;
use crate::ServiceState;

use super::MountInfoResponse;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateMountRequest {
    /// Enable/disable the mount
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Auto-mount on daemon startup
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_mount: Option<bool>,
    /// Mount in read-only mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only: Option<bool>,
    /// Cache size in MB
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_size_mb: Option<u32>,
    /// Cache TTL in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_ttl_secs: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMountResponse {
    pub mount: MountInfoResponse,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Path(mount_id): Path<Uuid>,
    Json(req): Json<UpdateMountRequest>,
) -> Result<impl IntoResponse, UpdateMountError> {
    let params = UpdateMountParams {
        enabled: req.enabled,
        auto_mount: req.auto_mount,
        read_only: req.read_only,
        cache_size_mb: req.cache_size_mb,
        cache_ttl_secs: req.cache_ttl_secs,
    };

    let mount = state
        .database()
        .update_mount(&mount_id, params)
        .await
        .map_err(|e| UpdateMountError::Database(e.to_string()))?
        .ok_or(UpdateMountError::NotFound(mount_id))?;

    Ok((
        http::StatusCode::OK,
        Json(UpdateMountResponse {
            mount: mount.into(),
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateMountError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Mount not found: {0}")]
    NotFound(Uuid),
}

impl IntoResponse for UpdateMountError {
    fn into_response(self) -> Response {
        let status = match &self {
            UpdateMountError::Database(_) => http::StatusCode::INTERNAL_SERVER_ERROR,
            UpdateMountError::NotFound(_) => http::StatusCode::NOT_FOUND,
        };
        (status, Json(serde_json::json!({ "error": self.to_string() }))).into_response()
    }
}

/// Request with mount_id for client-side use
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMountClientRequest {
    pub mount_id: Uuid,
    #[serde(flatten)]
    pub update: UpdateMountRequest,
}

impl ApiRequest for UpdateMountClientRequest {
    type Response = UpdateMountResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url
            .join(&format!("/api/v0/mounts/{}", self.mount_id))
            .unwrap();
        client.patch(full_url).json(&self.update)
    }
}
