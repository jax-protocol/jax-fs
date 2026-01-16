//! List configured mounts

use clap::Args;
use std::fmt;

use crate::daemon::http_server::api::client::ApiError;
use crate::daemon::http_server::api::v0::mounts::{ListMountsRequest, ListMountsResponse};
use crate::op::Op;

#[derive(Args, Debug, Clone)]
#[command(about = "List all configured FUSE mounts")]
pub struct List {
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug)]
pub struct ListOutput {
    response: ListMountsResponse,
    json: bool,
}

impl fmt::Display for ListOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.json {
            writeln!(f, "{}", serde_json::to_string_pretty(&self.response.mounts).unwrap_or_default())
        } else if self.response.mounts.is_empty() {
            writeln!(f, "No mounts configured. Use 'jax mount add' to create one.")
        } else {
            writeln!(f, "{:<36}  {:<36}  {:<30}  {:<10}  {:<10}",
                "MOUNT ID", "BUCKET ID", "MOUNT POINT", "STATUS", "AUTO-MOUNT")?;
            writeln!(f, "{}", "-".repeat(130))?;
            for mount in &self.response.mounts {
                let auto_mount = if mount.auto_mount { "yes" } else { "no" };
                writeln!(f, "{:<36}  {:<36}  {:<30}  {:<10}  {:<10}",
                    mount.mount_id,
                    mount.bucket_id,
                    truncate(&mount.mount_point, 30),
                    mount.status,
                    auto_mount
                )?;
            }
            Ok(())
        }
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("...{}", &s[s.len() - max_len + 3..])
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ListError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
}

#[async_trait::async_trait]
impl Op for List {
    type Error = ListError;
    type Output = ListOutput;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();
        let response: ListMountsResponse = client.call(ListMountsRequest::default()).await?;
        Ok(ListOutput {
            response,
            json: self.json,
        })
    }
}
