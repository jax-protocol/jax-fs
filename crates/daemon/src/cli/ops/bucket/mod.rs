use clap::{Args, Subcommand};

pub mod add;
pub mod approve;
pub mod cat;
pub mod clone;
pub mod clone_state;
pub mod create;
pub mod ignore;
pub mod list;
pub mod ls;
pub mod publish;
pub mod shares;
pub mod stat;
pub mod unpublish;

use crate::cli::op::Op;

crate::command_enum! {
    (Create, create::Create),
    (List, list::List),
    (Add, add::Add),
    (Ls, ls::Ls),
    (Cat, cat::Cat),
    (Shares, shares::Shares),
    (Stat, stat::Stat),
    (Clone, clone::Clone),
    (Publish, publish::Publish),
    (Unpublish, unpublish::Unpublish),
    (Approve, approve::Approve),
    (Ignore, ignore::Ignore),
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
