use std::fmt;

use clap::Args;
use owo_colors::OwoColorize;

use crate::cli::ui;
use jax_daemon::http_server::api::client::ApiError;
use jax_daemon::http_server::api::v0::bucket::list::{BucketInfo, ListRequest, ListResponse};

#[derive(Args, Debug, Clone)]
pub struct List {
    /// Filter buckets by name prefix
    #[arg(long)]
    pub prefix: Option<String>,

    /// Maximum number of buckets to return
    #[arg(long)]
    pub limit: Option<u32>,
}

#[derive(Debug)]
pub struct ListOutput {
    pub buckets: Vec<BucketInfo>,
}

impl fmt::Display for ListOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.buckets.is_empty() {
            return write!(f, "No buckets found");
        }

        let mut table = ui::styled_table(vec!["NAME", "ID", "LINK"]);
        for b in &self.buckets {
            table.add_row(vec![
                b.name.bold().to_string(),
                ui::truncate(&b.bucket_id.to_string(), 16),
                ui::truncate(&b.link.hash().to_string(), 16),
            ]);
        }
        write!(f, "{table}")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ListError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
}

#[async_trait::async_trait]
impl crate::cli::op::Op for List {
    type Error = ListError;
    type Output = ListOutput;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();
        let request = ListRequest {
            prefix: self.prefix.clone(),
            limit: self.limit,
        };
        let response: ListResponse = client.call(request).await?;

        Ok(ListOutput {
            buckets: response.buckets,
        })
    }
}
