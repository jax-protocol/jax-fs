use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use common::prelude::{Link, MountError};

use crate::http_server::api::client::ApiRequest;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MkdirRequest {
    pub bucket_id: Uuid,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MkdirResponse {
    pub path: String,
    pub link: Link,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(request): Json<MkdirRequest>,
) -> Result<impl IntoResponse, MkdirError> {
    let path = PathBuf::from(&request.path);

    // Load mount
    let mut mount = state.peer().mount(request.bucket_id).await?;

    // Create directory
    mount.mkdir(&path).await?;

    // Save mount
    let new_link = state.peer().save_mount(&mount, None).await?;

    Ok((
        http::StatusCode::OK,
        axum::Json(MkdirResponse {
            path: request.path,
            link: new_link,
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum MkdirError {
    #[error("Mount error: {0}")]
    Mount(#[from] MountError),
}

impl IntoResponse for MkdirError {
    fn into_response(self) -> axum::response::Response {
        (
            http::StatusCode::INTERNAL_SERVER_ERROR,
            "Unexpected error".to_string(),
        )
            .into_response()
    }
}

impl ApiRequest for MkdirRequest {
    type Response = MkdirResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket/mkdir").unwrap();
        client.post(full_url).json(&self)
    }
}
