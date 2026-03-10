use std::fmt;

use clap::Args;
use uuid::Uuid;

use crate::cli::op::{Op, OpContext};
use crate::cli::ui;
use jax_daemon::http_server::api::client::ApiError;
use jax_daemon::http_server::api::v0::mounts::{DeleteMountRequest, DeleteMountResponse};

#[derive(Args, Debug, Clone)]
pub struct Remove {
    /// Mount ID to remove
    pub id: Uuid,

    /// Force removal without confirmation
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Debug)]
pub struct RemoveOutput {
    pub mount_id: Uuid,
    pub deleted: bool,
}

impl fmt::Display for RemoveOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.deleted {
            write!(
                f,
                "{}",
                ui::success("Removed", &format!("mount {}", self.mount_id))
            )
        } else {
            write!(
                f,
                "{}",
                ui::failure("Not found", &format!("mount {}", self.mount_id))
            )
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

    async fn execute(&self, ctx: &OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();

        let request = DeleteMountRequest { mount_id: self.id };
        let response: DeleteMountResponse = client.call(request).await?;

        Ok(RemoveOutput {
            mount_id: self.id,
            deleted: response.deleted,
        })
    }
}
