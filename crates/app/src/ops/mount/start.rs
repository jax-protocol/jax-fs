//! Start a configured mount

use clap::Args;
use std::fmt;
use uuid::Uuid;

use crate::daemon::http_server::api::client::ApiError;
use crate::daemon::http_server::api::v0::mounts::start::{StartMountRequest, StartMountResponse};
use crate::op::Op;

#[derive(Args, Debug, Clone)]
#[command(about = "Start a configured FUSE mount")]
pub struct Start {
    /// Mount ID to start
    #[arg(index = 1)]
    pub mount_id: Uuid,
}

#[derive(Debug)]
pub struct StartOutput {
    response: StartMountResponse,
}

impl fmt::Display for StartOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Mount {} starting...", self.response.mount.mount_id)?;
        writeln!(f, "  Status: {}", self.response.mount.status)?;
        writeln!(f, "  Mount Point: {}", self.response.mount.mount_point)?;
        if let Some(pid) = self.response.mount.pid {
            writeln!(f, "  PID: {}", pid)?;
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
}

#[async_trait::async_trait]
impl Op for Start {
    type Error = StartError;
    type Output = StartOutput;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();

        let request = StartMountRequest {
            mount_id: self.mount_id,
        };

        let response: StartMountResponse = client.call(request).await?;

        Ok(StartOutput { response })
    }
}
