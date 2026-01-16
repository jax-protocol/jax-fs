//! Stop a running mount

use clap::Args;
use std::fmt;
use uuid::Uuid;

use crate::daemon::http_server::api::client::ApiError;
use crate::daemon::http_server::api::v0::mounts::stop::{StopMountRequest, StopMountResponse};
use crate::op::Op;

#[derive(Args, Debug, Clone)]
#[command(about = "Stop a running FUSE mount")]
pub struct Stop {
    /// Mount ID to stop
    #[arg(index = 1)]
    pub mount_id: Uuid,
}

#[derive(Debug)]
pub struct StopOutput {
    response: StopMountResponse,
}

impl fmt::Display for StopOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Mount {} stopping...", self.response.mount.mount_id)?;
        writeln!(f, "  Status: {}", self.response.mount.status)?;
        writeln!(f, "  Mount Point: {}", self.response.mount.mount_point)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StopError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
}

#[async_trait::async_trait]
impl Op for Stop {
    type Error = StopError;
    type Output = StopOutput;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();

        let request = StopMountRequest {
            mount_id: self.mount_id,
        };

        let response: StopMountResponse = client.call(request).await?;

        Ok(StopOutput { response })
    }
}
