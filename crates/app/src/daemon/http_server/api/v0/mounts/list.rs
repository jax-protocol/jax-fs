use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Json;
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::daemon::http_server::api::client::ApiRequest;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListMountsRequest {
    /// Filter by bucket ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bucket_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListMountsResponse {
    pub mounts: Vec<MountInfoResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountInfoResponse {
    pub mount_id: Uuid,
    pub bucket_id: Uuid,
    pub mount_point: String,
    pub enabled: bool,
    pub auto_mount: bool,
    pub read_only: bool,
    pub cache_size_mb: u32,
    pub cache_ttl_secs: u32,
    pub status: String,
    pub pid: Option<i64>,
    pub error_message: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

impl From<crate::daemon::database::mount_queries::MountInfo> for MountInfoResponse {
    fn from(m: crate::daemon::database::mount_queries::MountInfo) -> Self {
        Self {
            mount_id: m.mount_id,
            bucket_id: m.bucket_id,
            mount_point: m.mount_point,
            enabled: m.enabled,
            auto_mount: m.auto_mount,
            read_only: m.read_only,
            cache_size_mb: m.cache_size_mb,
            cache_ttl_secs: m.cache_ttl_secs,
            status: m.status.as_str().to_string(),
            pid: m.pid,
            error_message: m.error_message,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

pub async fn handler(
    State(state): State<ServiceState>,
) -> Result<impl IntoResponse, ListMountsError> {
    let mounts = state
        .database()
        .list_mounts()
        .await
        .map_err(|e| ListMountsError::Database(e.to_string()))?;

    let mount_infos: Vec<MountInfoResponse> = mounts.into_iter().map(Into::into).collect();

    Ok((
        http::StatusCode::OK,
        Json(ListMountsResponse {
            mounts: mount_infos,
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum ListMountsError {
    #[error("Database error: {0}")]
    Database(String),
}

impl IntoResponse for ListMountsError {
    fn into_response(self) -> Response {
        (
            http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": self.to_string() })),
        )
            .into_response()
    }
}

impl ApiRequest for ListMountsRequest {
    type Response = ListMountsResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/mounts").unwrap();
        client.get(full_url)
    }
}
