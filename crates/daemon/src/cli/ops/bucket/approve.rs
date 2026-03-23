use std::fmt;

use clap::Args;
use uuid::Uuid;

use crate::cli::ui;
use jax_daemon::http_server::api::client::{resolve_bucket, ApiError};
use jax_daemon::http_server::api::v0::bucket::approve::{ApproveRequest, ApproveResponse};

#[derive(Args, Debug, Clone)]
pub struct Approve {
    /// Bucket name or UUID
    pub bucket: String,
}

#[derive(Debug)]
pub struct ApproveOutput {
    pub bucket_id: Uuid,
    pub status: String,
}

impl fmt::Display for ApproveOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}",
            ui::success("Approved", &format!("bucket {}", self.bucket_id))
        )?;
        write!(f, "{}", ui::label("status", &self.status))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApproveError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
}

#[async_trait::async_trait]
impl crate::cli::op::Op for Approve {
    type Error = ApproveError;
    type Output = ApproveOutput;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();
        let bucket_id = resolve_bucket(&mut client, &self.bucket).await?;

        let request = ApproveRequest { bucket_id };
        let response: ApproveResponse = client.call(request).await?;

        Ok(ApproveOutput {
            bucket_id: response.bucket_id,
            status: response.status,
        })
    }
}
