use clap::{Args, Subcommand};

pub mod add;
pub mod cat;
pub mod clone;
pub mod clone_state;
pub mod create;
pub mod list;
pub mod ls;
pub mod shares;

use crate::cli::op::Op;
use jax_daemon::http_server::api::v0::bucket::{CreateRequest, ListRequest};

crate::command_enum! {
    (Create, CreateRequest),
    (List, ListRequest),
    (Add, add::Add),
    (Ls, ls::Ls),
    (Cat, cat::Cat),
    (Shares, shares::Shares),
    (Clone, clone::Clone),
}

// Rename the generated Command to BucketCommand for clarity
pub type BucketCommand = Command;

#[derive(Args, Debug, Clone)]
pub struct Bucket {
    #[command(subcommand)]
    pub command: BucketCommand,
}

#[async_trait::async_trait]
impl Op for Bucket {
    type Error = OpError;
    type Output = OpOutput;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        self.command.execute(ctx).await
    }
}
