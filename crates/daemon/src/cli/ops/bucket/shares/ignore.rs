use std::fmt;

use clap::Args;
use uuid::Uuid;

use crate::cli::ui;
use jax_daemon::http_server::api::client::{resolve_bucket, ApiError};
use jax_daemon::http_server::api::v0::bucket::ignore::{IgnoreRequest, IgnoreResponse};

#[derive(Args, Debug, Clone)]
pub struct Ignore {
    /// Bucket name or UUID
    pub bucket: String,
}

#[derive(Debug)]
pub struct IgnoreOutput {
    pub bucket_id: Uuid,
    pub status: String,
}

impl fmt::Display for IgnoreOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}",
            ui::warning(&format!("Ignored bucket {}", self.bucket_id))
        )?;
        write!(f, "{}", ui::label("status", &self.status))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IgnoreError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
}

#[async_trait::async_trait]
impl crate::cli::op::Op for Ignore {
    type Error = IgnoreError;
    type Output = IgnoreOutput;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();
        let bucket_id = resolve_bucket(&mut client, &self.bucket).await?;

        let request = IgnoreRequest { bucket_id };
        let response: IgnoreResponse = client.call(request).await?;

        Ok(IgnoreOutput {
            bucket_id: response.bucket_id,
            status: response.status,
        })
    }
}
