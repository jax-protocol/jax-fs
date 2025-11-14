use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use common::prelude::MountError;

use crate::daemon::http_server::api::client::ApiRequest;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize, clap::Args)]
pub struct CatRequest {
    /// Bucket ID to read from
    #[arg(long)]
    pub bucket_id: Uuid,

    /// Path in bucket to read
    #[arg(long)]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatResponse {
    pub path: String,
    /// Base64-encoded file content
    pub content: String,
    pub size: usize,
    pub mime_type: String,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<CatRequest>,
) -> Result<impl IntoResponse, CatError> {
    // Load mount at current head
    let mount = state.peer().mount(req.bucket_id).await?;

    let path_buf = std::path::PathBuf::from(&req.path);
    if !path_buf.is_absolute() {
        return Err(CatError::InvalidPath("Path must be absolute".into()));
    }

    // Get file data
    let data = mount.cat(&path_buf).await?;

    // Get node link to extract MIME type
    let node_link = mount.get(&path_buf).await?;
    let mime_type = node_link
        .data()
        .and_then(|data| data.mime())
        .map(|mime| mime.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());

    // Encode as base64 for JSON transport
    let content = base64::engine::general_purpose::STANDARD.encode(&data);
    let size = data.len();

    Ok((
        http::StatusCode::OK,
        Json(CatResponse {
            path: req.path,
            content,
            size,
            mime_type,
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum CatError {
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("Mount error: {0}")]
    Mount(#[from] MountError),
}

impl IntoResponse for CatError {
    fn into_response(self) -> Response {
        match self {
            CatError::InvalidPath(msg) => (
                http::StatusCode::BAD_REQUEST,
                format!("Invalid path: {}", msg),
            )
                .into_response(),
            CatError::Mount(_) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Unexpected error".to_string(),
            )
                .into_response(),
        }
    }
}

// Client implementation - builds request for this operation
impl ApiRequest for CatRequest {
    type Response = CatResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket/cat").unwrap();
        client.post(full_url).json(&self)
    }
}
