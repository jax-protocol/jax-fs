use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use common::prelude::{Link, MountError};

use crate::http_server::api::client::ApiRequest;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize, clap::Args)]
pub struct LsRequest {
    /// Bucket ID to list
    #[arg(long)]
    pub bucket_id: Uuid,

    /// Path in bucket to list (defaults to root)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub path: Option<String>,

    /// List recursively
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub deep: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LsResponse {
    pub items: Vec<PathInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathInfo {
    pub path: String,
    pub name: String,
    pub link: Link,
    pub is_dir: bool,
    pub mime_type: String,
}

#[axum::debug_handler]
pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<LsRequest>,
) -> Result<impl IntoResponse, LsError> {
    let deep = req.deep.unwrap_or(false);

    // Load mount based on role (owners see HEAD, mirrors see latest_published)
    let mount = state.peer().mount_for_read(req.bucket_id).await?;

    let path_str = req.path.as_deref().unwrap_or("/");
    let path_buf = std::path::PathBuf::from(path_str);

    // List items
    let items = if deep {
        mount.ls_deep(&path_buf).await?
    } else {
        mount.ls(&path_buf).await?
    };

    // Convert to response format
    let path_infos = items
        .into_iter()
        .map(|(path, node_link)| {
            // Mount returns relative paths, make them absolute
            let absolute_path = std::path::Path::new("/").join(&path);
            let path_str = absolute_path.to_string_lossy().to_string();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());

            let mime_type = if node_link.is_dir() {
                "inode/directory".to_string()
            } else {
                node_link
                    .data()
                    .and_then(|data| data.mime())
                    .map(|mime| mime.to_string())
                    .unwrap_or_else(|| "application/octet-stream".to_string())
            };

            PathInfo {
                path: path_str,
                name,
                link: node_link.link().clone(),
                is_dir: node_link.is_dir(),
                mime_type,
            }
        })
        .collect();

    Ok((http::StatusCode::OK, Json(LsResponse { items: path_infos })).into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum LsError {
    #[error("Mount error: {0}")]
    Mount(#[from] MountError),
}

impl IntoResponse for LsError {
    fn into_response(self) -> Response {
        (
            http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error: {}", self),
        )
            .into_response()
    }
}

// Client implementation - builds request for this operation
impl ApiRequest for LsRequest {
    type Response = LsResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket/ls").unwrap();
        client.post(full_url).json(&self)
    }
}
