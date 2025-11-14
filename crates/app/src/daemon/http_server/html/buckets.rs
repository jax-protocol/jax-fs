use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::Extension;
use tracing::instrument;

use crate::daemon::http_server::Config;
use crate::ServiceState;

#[derive(Template)]
#[template(path = "buckets.html")]
pub struct BucketsTemplate {
    pub buckets: Vec<BucketDisplayInfo>,
    pub read_only: bool,
    pub api_url: String,
}

#[derive(Debug, Clone)]
pub struct BucketDisplayInfo {
    pub bucket_id: String,
    pub name: String,
    pub created_at: String,
}

#[instrument(skip(state, config))]
pub async fn handler(
    State(state): State<ServiceState>,
    Extension(config): Extension<Config>,
    _headers: HeaderMap,
) -> askama_axum::Response {
    // Use the read_only flag from config
    let read_only = config.read_only;

    // Load buckets from database
    let buckets = match state.database().list_buckets(None, None).await {
        Ok(buckets) => buckets,
        Err(e) => {
            tracing::error!("Failed to list buckets: {}", e);
            return error_response("Failed to load buckets");
        }
    };

    // Convert to display format
    let display_buckets: Vec<BucketDisplayInfo> = buckets
        .into_iter()
        .map(|b| BucketDisplayInfo {
            bucket_id: b.id.to_string(),
            name: b.name,
            created_at: format_timestamp(b.created_at),
        })
        .collect();

    let api_url = config
        .api_url
        .clone()
        .unwrap_or_else(|| "http://localhost:3000".to_string());

    tracing::info!(
        "BUCKETS PAGE: API URL from config: {:?}, using: {}",
        config.api_url,
        api_url
    );

    let template = BucketsTemplate {
        buckets: display_buckets,
        read_only,
        api_url,
    };

    template.into_response()
}

fn format_timestamp(ts: time::OffsetDateTime) -> String {
    ts.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| ts.to_string())
}

fn error_response(message: &str) -> askama_axum::Response {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        format!("Error: {}", message),
    )
        .into_response()
}
