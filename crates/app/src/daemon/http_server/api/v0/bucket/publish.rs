use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use common::crypto::Secret;
use common::mount::PrincipalRole;
use common::prelude::MountError;
use serde::Serialize;
use uuid::Uuid;

use crate::ServiceState;

#[derive(Debug, Clone, Serialize)]
pub struct PublishResponse {
    pub bucket_id: Uuid,
    pub published: bool,
    pub mirrors_with_access: usize,
}

/// Publish a bucket, granting decryption access to all mirrors.
/// POST /api/v0/bucket/:bucket_id/publish
pub async fn handler(
    State(state): State<ServiceState>,
    Path(bucket_id): Path<Uuid>,
) -> Result<impl IntoResponse, PublishError> {
    tracing::info!("PUBLISH API: Publishing bucket {}", bucket_id);

    // Load mount at current head
    let mut mount = state.peer().mount(bucket_id).await?;
    tracing::info!("PUBLISH API: Loaded mount for bucket {}", bucket_id);

    // Check if already published
    if mount.is_published().await {
        tracing::info!("PUBLISH API: Bucket {} is already published", bucket_id);
        let inner = mount.inner().await;
        let mirrors_count = inner.manifest().get_shares_by_role(PrincipalRole::Mirror).len();
        return Ok((
            http::StatusCode::OK,
            axum::Json(PublishResponse {
                bucket_id,
                published: true,
                mirrors_with_access: mirrors_count,
            }),
        )
            .into_response());
    }

    // Generate a secret to grant access to mirrors.
    // This is a temporary secret - the real secret is generated during save().
    let secret = Secret::generate();
    mount.publish(&secret).await?;
    tracing::info!(
        "PUBLISH API: Granted access to mirrors for bucket {}",
        bucket_id
    );

    // Save mount to persist the changes
    let _new_link = state.peer().save_mount(&mount).await?;

    // Get mirror count
    let inner = mount.inner().await;
    let mirrors_count = inner.manifest().get_shares_by_role(PrincipalRole::Mirror).len();

    tracing::info!(
        "PUBLISH API: Bucket {} published, {} mirrors now have access",
        bucket_id,
        mirrors_count
    );

    Ok((
        http::StatusCode::OK,
        axum::Json(PublishResponse {
            bucket_id,
            published: true,
            mirrors_with_access: mirrors_count,
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    #[error("Mount error: {0}")]
    Mount(#[from] MountError),
}

impl IntoResponse for PublishError {
    fn into_response(self) -> Response {
        tracing::error!("PUBLISH ERROR: {:?}", self);
        match self {
            PublishError::Mount(e) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Mount error: {}", e),
            )
                .into_response(),
        }
    }
}
