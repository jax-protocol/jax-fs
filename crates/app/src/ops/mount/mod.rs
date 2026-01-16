//! Mount command - manage persistent FUSE mounts
//!
//! Top-level commands for managing bucket FUSE mounts:
//! - jax mount list           - List all configured mounts
//! - jax mount add            - Add a new mount configuration
//! - jax mount remove         - Remove a mount configuration
//! - jax mount start          - Start a configured mount
//! - jax mount stop           - Stop a running mount
//! - jax mount set            - Update mount configuration

use clap::{Args, Subcommand};

pub mod add;
pub mod list;
pub mod remove;
pub mod set;
pub mod start;
pub mod stop;

use crate::op::Op;

crate::command_enum! {
    (List, list::List),
    (Add, add::Add),
    (Remove, remove::Remove),
    (Start, start::Start),
    (Stop, stop::Stop),
    (Set, set::Set),
}

// Rename the generated Command to MountCommand for clarity
pub type MountCommand = Command;

#[derive(Args, Debug, Clone)]
#[command(about = "Manage persistent FUSE mounts for buckets")]
pub struct Mount {
    #[command(subcommand)]
    pub command: MountCommand,
}

#[async_trait::async_trait]
impl Op for Mount {
    type Error = OpError;
    type Output = OpOutput;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        self.command.execute(ctx).await
    }
}
