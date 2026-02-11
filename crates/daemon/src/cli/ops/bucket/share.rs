use jax_daemon::http_server::api::client::ApiError;
use jax_daemon::http_server::api::v0::bucket::share::{ShareRequest, ShareResponse};

#[derive(Debug, thiserror::Error)]
pub enum BucketShareError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
    #[error("Bucket share operation failed: {0}")]
    Failed(String),
}

#[async_trait::async_trait]
impl crate::cli::op::Op for ShareRequest {
    type Error = BucketShareError;
    type Output = String;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        // Always use API client
        let mut client = ctx.client.clone();
        let response: ShareResponse = client.call(self.clone()).await?;

        Ok(format!(
            "Bucket {} shared with peer {}. New bucket link: {}",
            response.bucket_id, response.peer_public_key, response.new_bucket_link
        ))
    }
}
