use reqwest::Client;
use url::Url;
use uuid::Uuid;

use super::error::ApiError;
use super::ApiRequest;
use crate::http_server::api::v0::bucket::list::{ListRequest, ListResponse};

#[derive(Debug, Clone)]
pub struct ApiClient {
    pub remote: Url,
    client: Client,
}

impl ApiClient {
    pub fn new(remote: &Url) -> Result<Self, ApiError> {
        let client = Client::builder().build()?;

        Ok(Self {
            remote: remote.clone(),
            client,
        })
    }

    pub async fn call<T: ApiRequest>(&mut self, request: T) -> Result<T::Response, ApiError> {
        let request_builder = request.build_request(&self.remote, &self.client);
        let response = request_builder.send().await?;

        if response.status().is_success() {
            Ok(response.json::<T::Response>().await?)
        } else {
            Err(ApiError::HttpStatus(
                response.status(),
                response.text().await?,
            ))
        }
    }

    /// Resolve a bucket name to a UUID
    /// Returns the first bucket with an exact name match
    pub async fn resolve_bucket_name(&mut self, name: &str) -> Result<Uuid, ApiError> {
        let request = ListRequest {
            prefix: Some(name.to_string()),
            limit: Some(100),
        };

        let response: ListResponse = self.call(request).await?;

        response
            .buckets
            .into_iter()
            .find(|b| b.name == name)
            .map(|b| b.bucket_id)
            .ok_or_else(|| {
                ApiError::HttpStatus(
                    reqwest::StatusCode::NOT_FOUND,
                    format!("Bucket not found: {}", name),
                )
            })
    }

    /// Get the base URL for API requests
    pub fn base_url(&self) -> &Url {
        &self.remote
    }

    /// Get the underlying HTTP client for custom requests
    pub fn http_client(&self) -> &Client {
        &self.client
    }
}

/// Resolve a bucket identifier (name or UUID string) to a UUID.
///
/// Tries to parse as UUID first; if that fails, resolves by name via the API.
pub async fn resolve_bucket(client: &mut ApiClient, identifier: &str) -> Result<Uuid, ApiError> {
    if let Ok(uuid) = Uuid::parse_str(identifier) {
        return Ok(uuid);
    }
    client.resolve_bucket_name(identifier).await
}
