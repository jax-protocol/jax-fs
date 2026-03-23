use clap::{Args, Subcommand};

pub mod create;
pub mod ls;
pub mod rm;

use crate::cli::op::Op;

crate::command_enum! {
    (Create, create::Create),
    (Ls, ls::Ls),
    (Rm, rm::Rm),
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
