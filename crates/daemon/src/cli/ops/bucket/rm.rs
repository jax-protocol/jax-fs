use std::fmt;

use clap::Args;
use uuid::Uuid;

use crate::cli::ui;
use jax_daemon::http_server::api::client::{resolve_bucket, ApiError};
use jax_daemon::http_server::api::v0::bucket::remove::{RemoveRequest, RemoveResponse};

#[derive(Args, Debug, Clone)]
pub struct Rm {
    /// Bucket name or UUID
    pub bucket: String,
}

#[derive(Debug)]
pub struct BucketRmOutput {
    pub bucket_id: Uuid,
    pub status: String,
}

impl fmt::Display for BucketRmOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}",
            ui::success("Removed", &format!("bucket {}", self.bucket_id))
        )?;
        write!(f, "{}", ui::label("status", &self.status))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BucketRmError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
}

#[async_trait::async_trait]
impl crate::cli::op::Op for Rm {
    type Error = BucketRmError;
    type Output = BucketRmOutput;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();
        let bucket_id = resolve_bucket(&mut client, &self.bucket).await?;

        let request = RemoveRequest { bucket_id };
        let response: RemoveResponse = client.call(request).await?;

        Ok(BucketRmOutput {
            bucket_id: response.bucket_id,
            status: response.status,
        })
    }
}
