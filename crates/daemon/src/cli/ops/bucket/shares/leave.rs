use std::fmt;

use clap::Args;
use uuid::Uuid;

use crate::cli::ui;
use jax_daemon::http_server::api::client::{resolve_bucket, ApiError};
use jax_daemon::http_server::api::v0::bucket::leave::{LeaveRequest, LeaveResponse};

#[derive(Args, Debug, Clone)]
pub struct Leave {
    /// Bucket name or UUID
    pub bucket: String,
}

#[derive(Debug)]
pub struct LeaveOutput {
    pub bucket_id: Uuid,
    pub status: String,
}

impl fmt::Display for LeaveOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}",
            ui::warning(&format!("Left bucket {}", self.bucket_id))
        )?;
        write!(f, "{}", ui::label("status", &self.status))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LeaveError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
}

#[async_trait::async_trait]
impl crate::cli::op::Op for Leave {
    type Error = LeaveError;
    type Output = LeaveOutput;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();
        let bucket_id = resolve_bucket(&mut client, &self.bucket).await?;

        let request = LeaveRequest { bucket_id };
        let response: LeaveResponse = client.call(request).await?;

        Ok(LeaveOutput {
            bucket_id: response.bucket_id,
            status: response.status,
        })
    }
}
