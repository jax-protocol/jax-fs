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
pub struct UnpublishRequest {
    /// Bucket ID to unpublish
    #[arg(long)]
    pub bucket_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnpublishResponse {
    pub bucket_id: Uuid,
    pub published: bool,
    pub new_bucket_link: String,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<UnpublishRequest>,
) -> Result<impl IntoResponse, UnpublishError> {
    tracing::info!("UNPUBLISH API: Unpublishing bucket {}", req.bucket_id);

    // Load mount at current head
    let mount = state.peer().mount(req.bucket_id).await?;

    // Check that the caller is the bucket owner
    let our_key = state.peer().secret().public();
    {
        let manifest = mount.inner().await;
        let our_share = manifest
            .manifest()
            .get_share(&our_key)
            .ok_or(UnpublishError::NotOwner)?;
        if *our_share.role() != PrincipalRole::Owner {
            return Err(UnpublishError::NotOwner);
        }
    }

    // Check if already unpublished
    if !mount.is_published().await {
        tracing::info!(
            "UNPUBLISH API: Bucket {} is already unpublished",
            req.bucket_id
        );
        let link = mount.link().await;
        return Ok((
            http::StatusCode::OK,
            Json(UnpublishResponse {
                bucket_id: req.bucket_id,
                published: false,
                new_bucket_link: link.hash().to_string(),
            }),
        )
            .into_response());
    }

    // Unpublish the bucket (clears publish state + saves + log + notify)
    let new_bucket_link = state.peer().unpublish_mount(&mount).await?;

    tracing::info!(
        "UNPUBLISH API: Bucket {} unpublished, new link: {}",
        req.bucket_id,
        new_bucket_link.hash()
    );

    Ok((
        http::StatusCode::OK,
        Json(UnpublishResponse {
            bucket_id: req.bucket_id,
            published: false,
            new_bucket_link: new_bucket_link.hash().to_string(),
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum UnpublishError {
    #[error("Mount error: {0}")]
    Mount(#[from] MountError),
    #[error("Only the bucket owner can unpublish")]
    NotOwner,
}

impl IntoResponse for UnpublishError {
    fn into_response(self) -> Response {
        match self {
            UnpublishError::Mount(_) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Unexpected error".to_string(),
            )
                .into_response(),
            UnpublishError::NotOwner => (
                http::StatusCode::FORBIDDEN,
                "Only the bucket owner can unpublish".to_string(),
            )
                .into_response(),
        }
    }
}

impl ApiRequest for UnpublishRequest {
    type Response = UnpublishResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket/unpublish").unwrap();
        client.post(full_url).json(&self)
    }
}
