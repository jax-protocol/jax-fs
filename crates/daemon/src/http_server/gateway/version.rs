use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use uuid::Uuid;

use common::bucket_log::BucketLogProvider;

use crate::ServiceState;

#[derive(Debug, Serialize)]
pub struct VersionResponse {
    pub bucket_id: Uuid,
    pub name: String,
    pub height: u64,
    pub link: String,
    pub published: bool,
    pub content_url: String,
}

pub async fn handler(State(state): State<ServiceState>, Path(bucket_id): Path<Uuid>) -> Response {
    // Check if the bucket exists
    let exists = match state.peer().logs().exists(bucket_id).await {
        Ok(exists) => exists,
        Err(e) => {
            tracing::error!("Failed to check bucket existence: {}", e);
            return super::error_response("Internal error");
        }
    };

    if !exists {
        return (axum::http::StatusCode::NOT_FOUND, "Bucket not found").into_response();
    }

    // Get latest published version
    let result = match state.peer().logs().latest_published(bucket_id).await {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("Failed to query latest published version: {}", e);
            return super::error_response("Internal error");
        }
    };

    let (link, height) = match result {
        Some((link, height)) => (link, height),
        None => {
            return (
                axum::http::StatusCode::NOT_FOUND,
                "No published version exists",
            )
                .into_response();
        }
    };

    // Get bucket name from database
    let name = match state.database().get_bucket_info(&bucket_id).await {
        Ok(Some(info)) => info.name,
        _ => String::new(),
    };

    let response = VersionResponse {
        bucket_id,
        name,
        height,
        link: link.to_string(),
        published: true,
        content_url: format!("/gw/{}/", bucket_id),
    };

    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        axum::Json(response),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_response_serializes_expected_fields() {
        let bucket_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let response = VersionResponse {
            bucket_id,
            name: "my-bucket".to_string(),
            height: 42,
            link: "bafy2bzacefake".to_string(),
            published: true,
            content_url: format!("/gw/{}/", bucket_id),
        };

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["bucket_id"], "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(json["name"], "my-bucket");
        assert_eq!(json["height"], 42);
        assert_eq!(json["link"], "bafy2bzacefake");
        assert_eq!(json["published"], true);
        assert_eq!(
            json["content_url"],
            "/gw/550e8400-e29b-41d4-a716-446655440000/"
        );
    }

    #[test]
    fn version_response_content_url_format() {
        let id = Uuid::new_v4();
        let response = VersionResponse {
            bucket_id: id,
            name: "test".to_string(),
            height: 1,
            link: "hash".to_string(),
            published: true,
            content_url: format!("/gw/{}/", id),
        };

        assert!(response.content_url.starts_with("/gw/"));
        assert!(response.content_url.ends_with('/'));
        assert!(response.content_url.contains(&id.to_string()));
    }
}
