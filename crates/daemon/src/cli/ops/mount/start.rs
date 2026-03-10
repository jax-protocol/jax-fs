use std::fmt;

use clap::Args;
use uuid::Uuid;

use crate::cli::op::{Op, OpContext};
use crate::cli::ui;
use jax_daemon::http_server::api::client::ApiError;
use jax_daemon::http_server::api::v0::mounts::{StartMountRequest, StartMountResponse};

#[derive(Args, Debug, Clone)]
pub struct Start {
    /// Mount ID to start
    pub id: Uuid,
}

#[derive(Debug)]
pub struct StartOutput {
    pub mount_id: Uuid,
    pub started: bool,
}

impl fmt::Display for StartOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.started {
            write!(
                f,
                "{}",
                ui::success("Started", &format!("mount {}", self.mount_id))
            )
        } else {
            write!(
                f,
                "{}",
                ui::failure("Failed", &format!("to start mount {}", self.mount_id))
            )
        }
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

    async fn execute(&self, ctx: &OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();

        let request = StartMountRequest { mount_id: self.id };
        let response: StartMountResponse = client.call(request).await?;

        Ok(StartOutput {
            mount_id: self.id,
            started: response.started,
        })
    }
}
