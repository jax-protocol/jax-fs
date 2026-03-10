use std::fmt;

use clap::Args;
use owo_colors::OwoColorize;
use uuid::Uuid;

use crate::cli::ui;
use jax_daemon::http_server::api::client::{resolve_bucket, ApiError};
use jax_daemon::http_server::api::v0::bucket::shares::{ShareInfo, SharesRequest, SharesResponse};

#[derive(Args, Debug, Clone)]
pub struct Ls {
    /// Bucket name or UUID
    pub bucket: String,
}

#[derive(Debug)]
pub struct SharesLsOutput {
    pub bucket_id: Uuid,
    pub shares: Vec<ShareInfo>,
}

impl fmt::Display for SharesLsOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.shares.is_empty() {
            return write!(f, "No shares for bucket {}", self.bucket_id.bold());
        }

        let mut table = ui::styled_table(vec!["KEY", "ROLE", ""]);
        for share in &self.shares {
            let marker = if share.is_self {
                "(you)".dimmed().to_string()
            } else {
                String::new()
            };
            table.add_row(vec![
                ui::truncate(&share.public_key, 24),
                ui::colored_role(&share.role),
                marker,
            ]);
        }
        write!(f, "{table}")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SharesLsError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
}

#[async_trait::async_trait]
impl crate::cli::op::Op for Ls {
    type Error = SharesLsError;
    type Output = SharesLsOutput;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();
        let bucket_id = resolve_bucket(&mut client, &self.bucket).await?;

        let request = SharesRequest { bucket_id };
        let response: SharesResponse = client.call(request).await?;

        Ok(SharesLsOutput {
            bucket_id: response.bucket_id,
            shares: response.shares,
        })
    }
}
