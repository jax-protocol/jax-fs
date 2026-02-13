//! List mounts API endpoint

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Json;
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};

use super::create::MountInfo;
use crate::http_server::api::client::ApiRequest;
use crate::ServiceState;

/// Request to list all mount configurations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListMountsRequest {}

/// Response containing all mount configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListMountsResponse {
    pub mounts: Vec<MountInfo>,
}

pub async fn handler(
    State(state): State<ServiceState>,
) -> Result<impl IntoResponse, ListMountsError> {
    let mount_manager = state.mount_manager().read().await;
    let mount_manager = mount_manager
        .as_ref()
        .ok_or(ListMountsError::MountManagerUnavailable)?;

    let mounts = mount_manager.list().await?;
    let mount_infos: Vec<MountInfo> = mounts.into_iter().map(Into::into).collect();

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
    #[error("Mount manager unavailable")]
    MountManagerUnavailable,
    #[error("Mount error: {0}")]
    Mount(#[from] crate::fuse::MountError),
}

impl IntoResponse for ListMountsError {
    fn into_response(self) -> Response {
        match self {
            ListMountsError::MountManagerUnavailable => (
                http::StatusCode::SERVICE_UNAVAILABLE,
                "Mount manager not available",
            )
                .into_response(),
            ListMountsError::Mount(e) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Mount error: {}", e),
            )
                .into_response(),
        }
    }
}

impl ApiRequest for ListMountsRequest {
    type Response = ListMountsResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/mounts/").unwrap();
        client.get(full_url)
    }
}
