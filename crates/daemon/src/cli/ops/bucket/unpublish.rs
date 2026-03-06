use std::fmt;

use clap::Args;
use owo_colors::OwoColorize;
use uuid::Uuid;

use jax_daemon::http_server::api::client::{resolve_bucket, ApiError};
use jax_daemon::http_server::api::v0::bucket::unpublish::{UnpublishRequest, UnpublishResponse};

#[derive(Args, Debug, Clone)]
pub struct Unpublish {
    /// Bucket name or UUID
    pub bucket: String,
}

#[derive(Debug)]
pub struct UnpublishOutput {
    pub bucket_id: Uuid,
    pub new_link: String,
}

impl fmt::Display for UnpublishOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{} bucket {}",
            "Unpublished".yellow().bold(),
            self.bucket_id.bold()
        )?;
        write!(f, "  {} {}", "link:".dimmed(), self.new_link)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UnpublishError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
}

#[async_trait::async_trait]
impl crate::cli::op::Op for Unpublish {
    type Error = UnpublishError;
    type Output = UnpublishOutput;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();
        let bucket_id = resolve_bucket(&mut client, &self.bucket).await?;

        let request = UnpublishRequest { bucket_id };
        let response: UnpublishResponse = client.call(request).await?;

        Ok(UnpublishOutput {
            bucket_id: response.bucket_id,
            new_link: response.new_bucket_link,
        })
    }
}
