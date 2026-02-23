use axum::response::{IntoResponse, Response};
use axum::Json;
use http::StatusCode;
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};

use common::version::BuildInfo;

use crate::http_server::api::client::ApiRequest;
use crate::version::build_info;

/// Request type for the version endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionRequest {}

impl ApiRequest for VersionRequest {
    type Response = BuildInfo;

    fn build_request(self, base_url: &Url, client: &Client) -> RequestBuilder {
        let full_url = base_url.join("/_status/version").unwrap();
        client.get(full_url)
    }
}

#[tracing::instrument]
pub async fn handler() -> Response {
    (StatusCode::OK, Json(build_info())).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_handler_direct() {
        let response = handler().await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn test_fuse_feature_detection() {
        let info = build_info();
        // When built with default features (which include fuse), BUILD_FEATURES should contain "fuse"
        #[cfg(feature = "fuse")]
        assert!(
            info.has_feature("fuse"),
            "Expected 'fuse' feature but got: {}",
            info.build_features
        );

        #[cfg(not(feature = "fuse"))]
        assert!(
            !info.has_feature("fuse"),
            "Did not expect 'fuse' feature but got: {}",
            info.build_features
        );
    }
}
