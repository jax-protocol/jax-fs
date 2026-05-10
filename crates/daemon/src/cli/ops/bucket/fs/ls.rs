use std::fmt;

use clap::Args;

use crate::cli::ui;
use jax_daemon::http_server::api::client::{resolve_bucket, ApiError};
use jax_daemon::http_server::api::v0::bucket::ls::{LsRequest, LsResponse, PathInfo};

#[derive(Args, Debug, Clone)]
pub struct Ls {
    /// Bucket name or UUID
    pub bucket: String,

    /// Path in bucket to list (defaults to root)
    #[arg(long)]
    pub path: Option<String>,

    /// List recursively
    #[arg(long)]
    pub deep: Option<bool>,
}

#[derive(Debug)]
pub struct LsOutput {
    pub items: Vec<PathInfo>,
}

impl fmt::Display for LsOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.items.is_empty() {
            return write!(f, "No items found");
        }

        let mut table = ui::styled_table(vec!["TYPE", "NAME", "HASH"]);
        for item in &self.items {
            let type_str = if item.is_dir { "dir" } else { "file" };
            table.add_row(vec![
                ui::colored_type(type_str),
                item.name.clone(),
                ui::truncate(&item.link.hash().to_string(), 16),
            ]);
        }
        write!(f, "{table}")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LsError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
}

#[async_trait::async_trait]
impl crate::cli::op::Op for Ls {
    type Error = LsError;
    type Output = LsOutput;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();
        let bucket_id = resolve_bucket(&mut client, &self.bucket).await?;

        let request = LsRequest {
            bucket_id,
            path: self.path.clone(),
            deep: self.deep,
            at: None,
        };

        let response: LsResponse = client.call(request).await?;

        Ok(LsOutput {
            items: response.items,
        })
    }
}
