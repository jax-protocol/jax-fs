//! Update mount configuration

use clap::Args;
use std::fmt;
use uuid::Uuid;

use crate::daemon::http_server::api::client::ApiError;
use crate::daemon::http_server::api::v0::mounts::update::{UpdateMountClientRequest, UpdateMountRequest, UpdateMountResponse};
use crate::op::Op;

#[derive(Args, Debug, Clone)]
#[command(about = "Update FUSE mount configuration")]
pub struct Set {
    /// Mount ID to update
    #[arg(index = 1)]
    pub mount_id: Uuid,

    /// Enable or disable the mount
    #[arg(long)]
    pub enabled: Option<bool>,

    /// Enable or disable auto-mount on daemon startup
    #[arg(long)]
    pub auto_mount: Option<bool>,

    /// Enable or disable read-only mode
    #[arg(long)]
    pub read_only: Option<bool>,

    /// Cache size in MB
    #[arg(long)]
    pub cache_size_mb: Option<u32>,

    /// Cache TTL in seconds
    #[arg(long)]
    pub cache_ttl_secs: Option<u32>,
}

#[derive(Debug)]
pub struct SetOutput {
    response: UpdateMountResponse,
}

impl fmt::Display for SetOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Mount {} updated successfully!", self.response.mount.mount_id)?;
        writeln!(f, "  Enabled:     {}", if self.response.mount.enabled { "yes" } else { "no" })?;
        writeln!(f, "  Auto-mount:  {}", if self.response.mount.auto_mount { "yes" } else { "no" })?;
        writeln!(f, "  Read-only:   {}", if self.response.mount.read_only { "yes" } else { "no" })?;
        writeln!(f, "  Cache Size:  {} MB", self.response.mount.cache_size_mb)?;
        writeln!(f, "  Cache TTL:   {} seconds", self.response.mount.cache_ttl_secs)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SetError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
    #[error("No options specified. Use --help to see available options.")]
    NoOptions,
}

#[async_trait::async_trait]
impl Op for Set {
    type Error = SetError;
    type Output = SetOutput;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        // Check if at least one option is specified
        if self.enabled.is_none()
            && self.auto_mount.is_none()
            && self.read_only.is_none()
            && self.cache_size_mb.is_none()
            && self.cache_ttl_secs.is_none()
        {
            return Err(SetError::NoOptions);
        }

        let mut client = ctx.client.clone();

        let request = UpdateMountClientRequest {
            mount_id: self.mount_id,
            update: UpdateMountRequest {
                enabled: self.enabled,
                auto_mount: self.auto_mount,
                read_only: self.read_only,
                cache_size_mb: self.cache_size_mb,
                cache_ttl_secs: self.cache_ttl_secs,
            },
        };

        let response: UpdateMountResponse = client.call(request).await?;

        Ok(SetOutput { response })
    }
}
