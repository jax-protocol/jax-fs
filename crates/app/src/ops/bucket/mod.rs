use clap::{Args, Subcommand};

pub mod add;
pub mod cat;
pub mod clone;
pub mod clone_state;
pub mod create;
#[cfg(feature = "fuse")]
pub mod fuse;
pub mod list;
pub mod ls;
pub mod share;
pub mod sync;

use crate::daemon::http_server::api::v0::bucket::{CreateRequest, ListRequest, ShareRequest};
use crate::op::Op;

#[cfg(not(feature = "fuse"))]
crate::command_enum! {
    (Create, CreateRequest),
    (List, ListRequest),
    (Add, add::Add),
    (Ls, ls::Ls),
    (Cat, cat::Cat),
    (Share, ShareRequest),
    (Clone, clone::Clone),
    (Sync, sync::Sync),
}

#[cfg(feature = "fuse")]
crate::command_enum! {
    (Create, CreateRequest),
    (List, ListRequest),
    (Add, add::Add),
    (Ls, ls::Ls),
    (Cat, cat::Cat),
    (Share, ShareRequest),
    (Clone, clone::Clone),
    (Sync, sync::Sync),
    (Fuse, fuse::Fuse),
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

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        self.command.execute(ctx).await
    }
}
