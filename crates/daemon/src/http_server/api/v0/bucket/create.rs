use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use common::bucket_log::BucketLogProvider;
use common::prelude::{Mount, MountError};

use crate::http_server::api::client::ApiRequest;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize, clap::Args)]
pub struct CreateRequest {
    /// Name of the bucket to create
    #[arg(long)]
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateResponse {
    pub bucket_id: Uuid,
    pub name: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<CreateRequest>,
) -> Result<impl IntoResponse, CreateError> {
    tracing::info!(
        "CREATE BUCKET: Received request to create bucket '{}'",
        req.name
    );

    // Validate bucket name
    if req.name.is_empty() {
        tracing::warn!("CREATE BUCKET: Invalid empty bucket name");
        return Err(CreateError::InvalidName("Name cannot be empty".into()));
    }

    let id = Uuid::new_v4();
    tracing::info!("CREATE BUCKET: Generated bucket ID: {}", id);

    let owner = state.node().secret();
    let blobs = state.node().blobs();

    tracing::info!("CREATE BUCKET: Initializing mount for bucket {}", id);
    let mount = Mount::init(id, req.name.clone(), owner, blobs)
        .await
        .map_err(|e| {
            tracing::error!("CREATE BUCKET: Failed to initialize mount: {}", e);
            e
        })?;
    tracing::info!(
        "CREATE BUCKET: Mount initialized successfully for bucket {}",
        id
    );

    // Get the genesis link from the mount
    let genesis_link = mount.link().await;
    tracing::info!(
        "CREATE BUCKET: Genesis link for bucket {}: {:?}",
        id,
        genesis_link
    );

    // Append genesis entry to log (height 0, no previous link, unpublished)
    tracing::info!(
        "CREATE BUCKET: Appending genesis entry to log for bucket {}",
        id
    );
    state
        .peer()
        .logs()
        .append(id, req.name.clone(), genesis_link.clone(), None, 0, false)
        .await
        .map_err(|e| {
            tracing::error!(
                "CREATE BUCKET: Failed to append genesis to log for bucket {}: {}",
                id,
                e
            );
            CreateError::SaveMount(format!("Failed to append genesis: {}", e))
        })?;
    tracing::info!(
        "CREATE BUCKET: Genesis entry appended successfully for bucket {}",
        id
    );

    tracing::info!(
        "CREATE BUCKET: Bucket '{}' created successfully with ID {}",
        req.name,
        id
    );
    Ok((
        http::StatusCode::CREATED,
        Json(CreateResponse {
            bucket_id: id,
            name: req.name,
            created_at: OffsetDateTime::now_utc(),
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum CreateError {
    #[error("Invalid bucket name: {0}")]
    InvalidName(String),
    #[error("Failed to save mount: {0}")]
    SaveMount(String),
    #[error("Mount error: {0}")]
    Mount(#[from] MountError),
}

impl IntoResponse for CreateError {
    fn into_response(self) -> Response {
        tracing::error!("CREATE BUCKET ERROR: {:?}", self);
        match self {
            CreateError::InvalidName(msg) => (
                http::StatusCode::BAD_REQUEST,
                format!("Invalid name: {}", msg),
            )
                .into_response(),
            CreateError::SaveMount(msg) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to save mount: {}", msg),
            )
                .into_response(),
            CreateError::Mount(e) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Mount error: {}", e),
            )
                .into_response(),
        }
    }
}

// Client implementation - builds request for this operation
impl ApiRequest for CreateRequest {
    type Response = CreateResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket").unwrap();
        client.post(full_url).json(&self)
    }
}
