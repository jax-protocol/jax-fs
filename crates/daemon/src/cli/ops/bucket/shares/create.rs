use std::fmt;

use clap::Args;
use uuid::Uuid;

use crate::cli::ui;
use jax_daemon::http_server::api::client::{resolve_bucket, ApiError};
use jax_daemon::http_server::api::v0::bucket::share::{ShareRequest, ShareResponse, ShareRole};

#[derive(Args, Debug, Clone)]
pub struct Create {
    /// Bucket name or UUID
    pub bucket: String,

    /// Public key of the peer to share with (hex-encoded)
    #[arg(long)]
    pub peer_public_key: String,

    /// Role for the peer (owner or mirror, defaults to owner)
    #[arg(long, default_value = "owner")]
    pub role: ShareRole,
}

#[derive(Debug)]
pub struct ShareCreateOutput {
    pub bucket_id: Uuid,
    pub peer_key: String,
    pub new_link: String,
}

impl fmt::Display for ShareCreateOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}",
            ui::success(
                "Shared",
                &format!("bucket {} with {}", self.bucket_id, self.peer_key)
            )
        )?;
        write!(f, "{}", ui::label("link", &self.new_link))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ShareCreateError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
}

#[async_trait::async_trait]
impl crate::cli::op::Op for Create {
    type Error = ShareCreateError;
    type Output = ShareCreateOutput;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();
        let bucket_id = resolve_bucket(&mut client, &self.bucket).await?;

        let request = ShareRequest {
            bucket_id,
            peer_public_key: self.peer_public_key.clone(),
            role: self.role,
        };
        let response: ShareResponse = client.call(request).await?;

        Ok(ShareCreateOutput {
            bucket_id: response.bucket_id,
            peer_key: response.peer_public_key,
            new_link: response.new_bucket_link,
        })
    }
}
