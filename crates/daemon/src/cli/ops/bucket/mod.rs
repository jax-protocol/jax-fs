use clap::{Args, Subcommand};

pub mod clone;
pub mod clone_state;
pub mod create;
pub mod fs;
pub mod list;
pub mod publish;
pub mod shares;
pub mod unpublish;

use crate::cli::op::Op;

crate::command_enum! {
    (Create, create::Create),
    (Ls, list::List),
    (Fs, fs::Fs),
    (Shares, shares::Shares),
    (Clone, clone::Clone),
    (Publish, publish::Publish),
    (Unpublish, unpublish::Unpublish),
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
