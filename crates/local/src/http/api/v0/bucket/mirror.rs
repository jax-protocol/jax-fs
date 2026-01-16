use axum::extract::{Json, Path, State};
use axum::response::{IntoResponse, Response};
use common::prelude::MountError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use common::crypto::PublicKey;

use service::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddMirrorRequest {
    /// Public key of the mirror peer (base64 encoded)
    pub public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddMirrorResponse {
    pub bucket_id: Uuid,
    pub mirror_added: String,
    pub total_mirrors: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveMirrorResponse {
    pub bucket_id: Uuid,
    pub mirror_removed: String,
    pub total_mirrors: usize,
}

/// Add a mirror peer to a bucket
/// POST /api/v0/bucket/:bucket_id/mirror
pub async fn add_handler(
    State(state): State<ServiceState>,
    Path(bucket_id): Path<Uuid>,
    Json(req): Json<AddMirrorRequest>,
) -> Result<impl IntoResponse, MirrorError> {
    tracing::info!(
        "MIRROR API: Adding mirror {} to bucket {}",
        req.public_key,
        bucket_id
    );

    // Parse the peer's public key from hex (or base64)
    let public_key = parse_public_key(&req.public_key)?;

    // Load mount at current head
    let mut mount = state.peer().mount(bucket_id).await?;
    tracing::info!("MIRROR API: Loaded mount for bucket {}", bucket_id);

    // Add mirror to the bucket
    mount.add_mirror(public_key).await;
    tracing::info!("MIRROR API: Added mirror {} to bucket", req.public_key);

    // Save mount and update log
    let _new_link = state.peer().save_mount(&mount).await?;

    // Get updated mirror count
    let inner = mount.inner().await;
    let total_mirrors = inner.manifest().get_mirrors().len();

    tracing::info!(
        "MIRROR API: Bucket {} now has {} mirrors",
        bucket_id,
        total_mirrors
    );

    Ok((
        http::StatusCode::OK,
        Json(AddMirrorResponse {
            bucket_id,
            mirror_added: req.public_key,
            total_mirrors,
        }),
    )
        .into_response())
}

/// Remove a mirror peer from a bucket
/// DELETE /api/v0/bucket/:bucket_id/mirror/:public_key
pub async fn remove_handler(
    State(state): State<ServiceState>,
    Path((bucket_id, public_key_str)): Path<(Uuid, String)>,
) -> Result<impl IntoResponse, MirrorError> {
    tracing::info!(
        "MIRROR API: Removing mirror {} from bucket {}",
        public_key_str,
        bucket_id
    );

    // Parse the peer's public key
    let public_key = parse_public_key(&public_key_str)?;

    // Load mount at current head
    let mut mount = state.peer().mount(bucket_id).await?;
    tracing::info!("MIRROR API: Loaded mount for bucket {}", bucket_id);

    // Remove mirror from the bucket
    let removed = mount.remove_mirror(&public_key).await;
    if !removed {
        return Err(MirrorError::MirrorNotFound(public_key_str.clone()));
    }
    tracing::info!(
        "MIRROR API: Removed mirror {} from bucket",
        public_key_str
    );

    // Save mount and update log
    let _new_link = state.peer().save_mount(&mount).await?;

    // Get updated mirror count
    let inner = mount.inner().await;
    let total_mirrors = inner.manifest().get_mirrors().len();

    tracing::info!(
        "MIRROR API: Bucket {} now has {} mirrors",
        bucket_id,
        total_mirrors
    );

    Ok((
        http::StatusCode::OK,
        Json(RemoveMirrorResponse {
            bucket_id,
            mirror_removed: public_key_str,
            total_mirrors,
        }),
    )
        .into_response())
}

/// Parse a public key from hex or base64 encoding
fn parse_public_key(key_str: &str) -> Result<PublicKey, MirrorError> {
    // Try hex first (64 chars for 32 bytes)
    if key_str.len() == 64 && key_str.chars().all(|c| c.is_ascii_hexdigit()) {
        return PublicKey::from_hex(key_str)
            .map_err(|e| MirrorError::InvalidPublicKey(e.to_string()));
    }

    // Try base64
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, key_str)
        .map_err(|e| MirrorError::InvalidPublicKey(format!("Invalid base64: {}", e)))?;

    if bytes.len() != 32 {
        return Err(MirrorError::InvalidPublicKey(format!(
            "Expected 32 bytes, got {}",
            bytes.len()
        )));
    }

    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(PublicKey::from(arr))
}

#[derive(Debug, thiserror::Error)]
pub enum MirrorError {
    #[error("Invalid public key: {0}")]
    InvalidPublicKey(String),
    #[error("Mirror not found: {0}")]
    MirrorNotFound(String),
    #[error("Mount error: {0}")]
    Mount(#[from] MountError),
}

impl IntoResponse for MirrorError {
    fn into_response(self) -> Response {
        tracing::error!("MIRROR ERROR: {:?}", self);
        match self {
            MirrorError::InvalidPublicKey(msg) => (
                http::StatusCode::BAD_REQUEST,
                format!("Invalid public key: {}", msg),
            )
                .into_response(),
            MirrorError::MirrorNotFound(key) => (
                http::StatusCode::NOT_FOUND,
                format!("Mirror not found: {}", key),
            )
                .into_response(),
            MirrorError::Mount(e) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Mount error: {}", e),
            )
                .into_response(),
        }
    }
}
