use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::{Path, State};
use tracing::instrument;
use uuid::Uuid;

use crate::ServiceState;

#[derive(Template)]
#[template(path = "pins_explorer.html")]
pub struct PinsExplorerTemplate {
    pub bucket_id: String,
    pub bucket_name: String,
    pub pins: Vec<PinInfo>,
    pub total_pins: usize,
}

#[derive(Debug, Clone)]
pub struct PinInfo {
    pub hash: String,
}

#[instrument(skip(state))]
pub async fn handler(
    State(state): State<ServiceState>,
    Path(bucket_id): Path<Uuid>,
) -> askama_axum::Response {
    // Get bucket info from bucket_log
    let bucket = match state.database().get_bucket_info(&bucket_id).await {
        Ok(Some(bucket)) => bucket,
        Ok(None) => return error_response("Bucket not found"),
        Err(e) => return error_response(&format!("Failed to load bucket: {}", e)),
    };

    // Load mount and get pins
    let mount = match state.peer().mount(bucket_id).await {
        Ok(mount) => mount,
        Err(e) => {
            tracing::error!("Failed to load mount: {}", e);
            return error_response("Failed to load bucket");
        }
    };

    let inner = mount.inner().await;
    let pins_hashes: Vec<_> = inner.pins().iter().cloned().collect();

    let total_pins = pins_hashes.len();

    // Convert to display format
    let pins: Vec<PinInfo> = pins_hashes
        .into_iter()
        .map(|hash| PinInfo {
            hash: hash.to_string(),
        })
        .collect();

    let template = PinsExplorerTemplate {
        bucket_id: bucket_id.to_string(),
        bucket_name: bucket.name,
        pins,
        total_pins,
    };

    template.into_response()
}

fn error_response(message: &str) -> askama_axum::Response {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        format!("Error: {}", message),
    )
        .into_response()
}
