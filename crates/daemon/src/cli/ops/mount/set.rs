use std::fmt;

use clap::Args;
use uuid::Uuid;

use crate::cli::op::{Op, OpContext};
use crate::cli::ui;
use jax_daemon::http_server::api::client::ApiError;
use jax_daemon::http_server::api::v0::mounts::{
    UpdateMountBody, UpdateMountRequest, UpdateMountResponse,
};

#[derive(Args, Debug, Clone)]
pub struct Set {
    /// Mount ID to update
    pub id: Uuid,

    /// New mount point path
    #[arg(long)]
    pub mount_point: Option<String>,

    /// Enable/disable mount
    #[arg(long)]
    pub enabled: Option<bool>,

    /// Enable/disable auto-mount
    #[arg(long)]
    pub auto_mount: Option<bool>,

    /// Enable/disable read-only mode
    #[arg(long)]
    pub read_only: Option<bool>,

    /// Cache size in MB
    #[arg(long)]
    pub cache_size: Option<u32>,

    /// Cache TTL in seconds
    #[arg(long)]
    pub cache_ttl: Option<u32>,
}

#[derive(Debug)]
pub struct SetOutput {
    pub mount_id: Uuid,
    pub mount_point: String,
    pub enabled: bool,
    pub auto_mount: bool,
    pub read_only: bool,
    pub cache_size_mb: u32,
    pub cache_ttl_secs: u32,
}

impl fmt::Display for SetOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}",
            ui::success("Updated", &format!("mount {}", self.mount_id))
        )?;
        writeln!(f, "{}", ui::label("mount_point", &self.mount_point))?;
        writeln!(f, "{}", ui::label("enabled", &self.enabled))?;
        writeln!(f, "{}", ui::label("auto_mount", &self.auto_mount))?;
        writeln!(f, "{}", ui::label("read_only", &self.read_only))?;
        writeln!(f, "{}", ui::label("cache_size_mb", &self.cache_size_mb))?;
        write!(f, "{}", ui::label("cache_ttl_secs", &self.cache_ttl_secs))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SetError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
}

#[async_trait::async_trait]
impl Op for Set {
    type Error = SetError;
    type Output = SetOutput;

    async fn execute(&self, ctx: &OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();

        let request = UpdateMountRequest {
            mount_id: self.id,
            body: UpdateMountBody {
                mount_point: self.mount_point.clone(),
                enabled: self.enabled,
                auto_mount: self.auto_mount,
                read_only: self.read_only,
                cache_size_mb: self.cache_size,
                cache_ttl_secs: self.cache_ttl,
            },
        };

        let response: UpdateMountResponse = client.call(request).await?;

        Ok(SetOutput {
            mount_id: response.mount.mount_id,
            mount_point: response.mount.mount_point,
            enabled: response.mount.enabled,
            auto_mount: response.mount.auto_mount,
            read_only: response.mount.read_only,
            cache_size_mb: response.mount.cache_size_mb,
            cache_ttl_secs: response.mount.cache_ttl_secs,
        })
    }
}
