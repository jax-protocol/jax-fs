use clap::{Args, Subcommand};

pub mod create;
pub mod ls;

use crate::cli::op::Op;
use jax_daemon::http_server::api::v0::bucket::{ShareRequest, SharesRequest};

crate::command_enum! {
    (Create, ShareRequest),
    (Ls, SharesRequest),
}

// Rename the generated Command to SharesCommand for clarity
pub type SharesCommand = Command;

#[derive(Args, Debug, Clone)]
pub struct Shares {
    #[command(subcommand)]
    pub command: SharesCommand,
}

#[async_trait::async_trait]
impl Op for Shares {
    type Error = OpError;
    type Output = OpOutput;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        self.command.execute(ctx).await
    }
}
