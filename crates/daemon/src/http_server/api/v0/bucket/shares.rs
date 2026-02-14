use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use common::mount::PrincipalRole;
use common::prelude::MountError;
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::http_server::api::client::ApiRequest;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize, clap::Args)]
pub struct SharesRequest {
    /// Bucket ID to list shares for
    #[arg(long)]
    pub bucket_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharesResponse {
    pub bucket_id: Uuid,
    pub self_key: String,
    pub shares: Vec<ShareInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareInfo {
    pub public_key: String,
    pub role: String,
    pub is_self: bool,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<SharesRequest>,
) -> Result<impl IntoResponse, SharesError> {
    let mount = state.peer().mount_for_read(req.bucket_id).await?;
    let inner = mount.inner().await;
    let manifest_shares = inner.manifest().shares();

    let self_key = state.peer().secret().public().to_hex();

    let shares: Vec<ShareInfo> = manifest_shares
        .iter()
        .map(|(key_hex, share)| {
            let role = match share.role() {
                PrincipalRole::Owner => "Owner",
                PrincipalRole::Mirror => "Mirror",
            };
            ShareInfo {
                public_key: key_hex.clone(),
                role: role.to_string(),
                is_self: *key_hex == self_key,
            }
        })
        .collect();

    Ok((
        http::StatusCode::OK,
        Json(SharesResponse {
            bucket_id: req.bucket_id,
            self_key,
            shares,
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum SharesError {
    #[error("Mount error: {0}")]
    Mount(#[from] MountError),
}

impl IntoResponse for SharesError {
    fn into_response(self) -> Response {
        (
            http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error: {}", self),
        )
            .into_response()
    }
}

impl ApiRequest for SharesRequest {
    type Response = SharesResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket/shares").unwrap();
        client.post(full_url).json(&self)
    }
}
