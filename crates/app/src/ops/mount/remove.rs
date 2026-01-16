//! Remove a mount configuration

use clap::Args;
use std::fmt;
use uuid::Uuid;

use crate::daemon::http_server::api::client::ApiError;
use crate::daemon::http_server::api::v0::mounts::delete_mount::{DeleteMountRequest, DeleteMountResponse};
use crate::op::Op;

#[derive(Args, Debug, Clone)]
#[command(about = "Remove a FUSE mount configuration")]
pub struct Remove {
    /// Mount ID to remove
    #[arg(index = 1)]
    pub mount_id: Uuid,

    /// Force removal even if mount might be running (will attempt to stop first)
    #[arg(long, short = 'f')]
    pub force: bool,
}

#[derive(Debug)]
pub struct RemoveOutput {
    mount_id: Uuid,
    deleted: bool,
}

impl fmt::Display for RemoveOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.deleted {
            writeln!(f, "Mount {} removed successfully.", self.mount_id)
        } else {
            writeln!(f, "Mount {} was not found or could not be removed.", self.mount_id)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RemoveError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
}

#[async_trait::async_trait]
impl Op for Remove {
    type Error = RemoveError;
    type Output = RemoveOutput;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();

        // TODO: If --force, try to stop the mount first before deleting

        let request = DeleteMountRequest {
            mount_id: self.mount_id,
        };

        let response: DeleteMountResponse = client.call(request).await?;

        Ok(RemoveOutput {
            mount_id: self.mount_id,
            deleted: response.deleted,
        })
    }
}
