//! Add a new mount configuration

use clap::Args;
use std::fmt;
use std::path::PathBuf;
use uuid::Uuid;

use crate::daemon::http_server::api::client::ApiError;
use crate::daemon::http_server::api::v0::mounts::{CreateMountRequest, CreateMountResponse};
use crate::op::Op;

#[derive(Args, Debug, Clone)]
#[command(about = "Add a new FUSE mount configuration")]
pub struct Add {
    /// Bucket name or ID to mount
    #[arg(index = 1)]
    pub bucket: String,

    /// Local directory to mount the bucket at
    #[arg(index = 2)]
    pub mount_point: PathBuf,

    /// Auto-mount on daemon startup
    #[arg(long)]
    pub auto_mount: bool,

    /// Mount in read-only mode
    #[arg(long)]
    pub read_only: bool,

    /// Cache size in MB (default: 100)
    #[arg(long, default_value = "100")]
    pub cache_size_mb: u32,

    /// Cache TTL in seconds (default: 60)
    #[arg(long, default_value = "60")]
    pub cache_ttl_secs: u32,

    /// Start the mount immediately after adding
    #[arg(long, short = 's')]
    pub start: bool,
}

#[derive(Debug)]
pub struct AddOutput {
    response: CreateMountResponse,
}

impl fmt::Display for AddOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Mount created successfully!")?;
        writeln!(f, "  Mount ID:    {}", self.response.mount.mount_id)?;
        writeln!(f, "  Bucket ID:   {}", self.response.mount.bucket_id)?;
        writeln!(f, "  Mount Point: {}", self.response.mount.mount_point)?;
        writeln!(f, "  Auto-mount:  {}", if self.response.mount.auto_mount { "yes" } else { "no" })?;
        writeln!(f, "  Read-only:   {}", if self.response.mount.read_only { "yes" } else { "no" })?;
        writeln!(f)?;
        writeln!(f, "Use 'jax mount start {}' to start the mount.", self.response.mount.mount_id)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AddError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
}

#[async_trait::async_trait]
impl Op for Add {
    type Error = AddError;
    type Output = AddOutput;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();

        // Resolve bucket name to ID if needed
        let bucket_id = if let Ok(id) = self.bucket.parse::<Uuid>() {
            id
        } else {
            client.resolve_bucket_name(&self.bucket).await?
        };

        let request = CreateMountRequest {
            bucket_id,
            mount_point: self.mount_point.to_string_lossy().to_string(),
            auto_mount: self.auto_mount,
            read_only: self.read_only,
            cache_size_mb: Some(self.cache_size_mb),
            cache_ttl_secs: Some(self.cache_ttl_secs),
        };

        let response: CreateMountResponse = client.call(request).await?;

        // TODO: If --start was specified, also start the mount
        // This would require calling the start endpoint after creating

        Ok(AddOutput { response })
    }
}
