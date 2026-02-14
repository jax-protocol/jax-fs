use jax_daemon::http_server::api::client::ApiError;
use jax_daemon::http_server::api::v0::bucket::shares::{SharesRequest, SharesResponse};

#[derive(Debug, thiserror::Error)]
pub enum BucketSharesError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
}

#[async_trait::async_trait]
impl crate::cli::op::Op for SharesRequest {
    type Error = BucketSharesError;
    type Output = String;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();
        let response: SharesResponse = client.call(self.clone()).await?;

        if response.shares.is_empty() {
            return Ok(format!("No shares for bucket {}", response.bucket_id));
        }

        let mut lines = Vec::new();
        lines.push(format!("Shares for bucket {}:", response.bucket_id));
        for share in &response.shares {
            let marker = if share.is_self { " (you)" } else { "" };
            lines.push(format!("  {}  {}{marker}", share.public_key, share.role));
        }
        Ok(lines.join("\n"))
    }
}
