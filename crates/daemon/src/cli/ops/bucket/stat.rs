use std::fmt;

use clap::Args;
use comfy_table::Table;

use jax_daemon::http_server::api::client::{resolve_bucket, ApiError};
use jax_daemon::http_server::api::v0::bucket::stat::{StatRequest, StatResponse};

#[derive(Args, Debug, Clone)]
pub struct Stat {
    /// Bucket name or UUID
    pub bucket: String,
}

#[derive(Debug)]
pub struct StatOutput {
    pub response: StatResponse,
}

impl fmt::Display for StatOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = &self.response;
        writeln!(f, "Bucket:    {}", r.name)?;
        writeln!(f, "ID:        {}", r.bucket_id)?;
        writeln!(f, "Version:   {}", r.link.hash())?;
        writeln!(f, "Height:    {}", r.height)?;
        writeln!(
            f,
            "Published: {}",
            if r.published { "yes" } else { "no" }
        )?;

        if !r.peers.is_empty() {
            writeln!(f)?;
            let mut table = Table::new();
            table.set_header(vec!["PEER", "ROLE", ""]);
            for p in &r.peers {
                table.add_row(vec![
                    p.public_key.clone(),
                    p.role.clone(),
                    if p.is_self {
                        "(you)".to_string()
                    } else {
                        String::new()
                    },
                ]);
            }
            write!(f, "{table}")?;
        }

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StatError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
}

#[async_trait::async_trait]
impl crate::cli::op::Op for Stat {
    type Error = StatError;
    type Output = StatOutput;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();
        let bucket_id = resolve_bucket(&mut client, &self.bucket).await?;
        let request = StatRequest { bucket_id };
        let response: StatResponse = client.call(request).await?;

        Ok(StatOutput { response })
    }
}
