use clap::{Args, Subcommand};

pub mod add;
pub mod cat;
pub mod ls;
pub mod stat;

use crate::cli::op::Op;

crate::command_enum! {
    (Ls, ls::Ls),
    (Cat, cat::Cat),
    (Add, add::Add),
    (Stat, stat::Stat),
}

// Rename the generated Command to FsCommand for clarity
pub type FsCommand = Command;

#[derive(Args, Debug, Clone)]
pub struct Fs {
    #[command(subcommand)]
    pub command: FsCommand,
}

#[async_trait::async_trait]
impl Op for Fs {
    type Error = OpError;
    type Output = OpOutput;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        self.command.execute(ctx).await
    }
}
