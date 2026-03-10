use std::fmt;

use clap::Args;
use uuid::Uuid;

use crate::cli::op::{Op, OpContext};
use crate::cli::ui;
use jax_daemon::http_server::api::client::{resolve_bucket, ApiError};
use jax_daemon::http_server::api::v0::mounts::{CreateMountRequest, CreateMountResponse};

#[derive(Args, Debug, Clone)]
pub struct Add {
    /// Bucket name or ID
    pub bucket: String,

    /// Mount point path
    pub path: String,

    /// Auto-mount on daemon startup
    #[arg(long)]
    pub auto_mount: bool,

    /// Mount as read-only
    #[arg(long)]
    pub read_only: bool,

    /// Cache size in MB
    #[arg(long, default_value = "100")]
    pub cache_size: u32,

    /// Cache TTL in seconds
    #[arg(long, default_value = "60")]
    pub cache_ttl: u32,
}

#[derive(Debug)]
pub struct AddOutput {
    pub mount_id: Uuid,
    pub bucket_id: Uuid,
    pub path: String,
    pub status: String,
}

impl fmt::Display for AddOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}",
            ui::success("Created", &format!("mount {}", self.mount_id))
        )?;
        writeln!(f, "{}", ui::label("bucket", &self.bucket_id))?;
        writeln!(f, "{}", ui::label("path", &self.path))?;
        write!(
            f,
            "{}",
            ui::label("status", &ui::colored_status(&self.status))
        )
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

    async fn execute(&self, ctx: &OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();
        let bucket_id = resolve_bucket(&mut client, &self.bucket).await?;

        let request = CreateMountRequest {
            bucket_id,
            mount_point: self.path.clone(),
            auto_mount: self.auto_mount,
            read_only: self.read_only,
            cache_size_mb: Some(self.cache_size),
            cache_ttl_secs: Some(self.cache_ttl),
        };

        let response: CreateMountResponse = client.call(request).await?;

        Ok(AddOutput {
            mount_id: response.mount.mount_id,
            bucket_id: response.mount.bucket_id,
            path: response.mount.mount_point,
            status: response.mount.status,
        })
    }
}
