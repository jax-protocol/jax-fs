use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use common::prelude::Link;

use crate::http_server::api::client::ApiRequest;
use crate::ServiceState;

#[derive(Debug, Clone, Serialize, Deserialize, clap::Args)]
pub struct ListRequest {
    /// Optional prefix filter
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub prefix: Option<String>,

    /// Optional limit
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub limit: Option<u32>,

    /// Optional status filter (pending, active, ignored)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResponse {
    pub buckets: Vec<BucketInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketInfo {
    pub bucket_id: Uuid,
    pub name: String,
    pub link: Link,
    pub status: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Json(req): Json<ListRequest>,
) -> Result<impl IntoResponse, ListError> {
    // Query buckets from bucket_log
    let buckets = state
        .database()
        .list_buckets(req.prefix, req.limit)
        .await
        .map_err(|e| ListError::Database(e.to_string()))?;

    // Parse optional status filter
    let status_filter = req.status.as_deref();

    // Convert to response format, adding status from bucket_status table
    let mut bucket_infos = Vec::new();
    for b in buckets {
        let status = state
            .database()
            .get_effective_bucket_status(&b.id)
            .await
            .map_err(|e| ListError::Database(e.to_string()))?;

        let status_str = status.as_str();

        // Apply status filter if provided
        if let Some(filter) = status_filter {
            if status_str != filter {
                continue;
            }
        }

        bucket_infos.push(BucketInfo {
            bucket_id: b.id,
            name: b.name,
            link: b.link,
            status: status_str.to_string(),
            created_at: b.created_at,
        });
    }

    Ok((
        http::StatusCode::OK,
        Json(ListResponse {
            buckets: bucket_infos,
        }),
    )
        .into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum ListError {
    #[error("Database error: {0}")]
    Database(String),
}

impl IntoResponse for ListError {
    fn into_response(self) -> Response {
        (
            http::StatusCode::INTERNAL_SERVER_ERROR,
            "unknown server error",
        )
            .into_response()
    }
}

// Client implementation - builds request for this operation
impl ApiRequest for ListRequest {
    type Response = ListResponse;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/api/v0/bucket/list").unwrap();
        client.post(full_url).json(&self)
    }
}
