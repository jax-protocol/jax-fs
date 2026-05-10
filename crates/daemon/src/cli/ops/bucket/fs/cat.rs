use std::fmt;

use base64::Engine;
use clap::Args;

use crate::cli::ui;
use jax_daemon::http_server::api::client::{resolve_bucket, ApiError};
use jax_daemon::http_server::api::v0::bucket::cat::{CatRequest, CatResponse};

#[derive(Args, Debug, Clone)]
pub struct Cat {
    /// Bucket name or UUID
    pub bucket: String,

    /// Path in bucket to read
    pub path: String,
}

#[derive(Debug)]
pub enum CatContent {
    Text(String),
    Binary(Vec<u8>),
}

#[derive(Debug)]
pub struct CatOutput {
    pub path: String,
    pub size: usize,
    pub content: CatContent,
}

impl fmt::Display for CatOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}  {}",
            ui::label("File", &self.path),
            ui::label("Size", &format!("{} bytes", self.size)),
        )?;
        match &self.content {
            CatContent::Text(text) => write!(f, "{text}"),
            CatContent::Binary(bytes) => {
                let hex = bytes
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                write!(f, "{}", ui::label("Binary content (hex)", &hex))
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CatError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
    #[error("Base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
}

#[async_trait::async_trait]
impl crate::cli::op::Op for Cat {
    type Error = CatError;
    type Output = CatOutput;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();
        let bucket_id = resolve_bucket(&mut client, &self.bucket).await?;

        let request = CatRequest {
            bucket_id,
            path: self.path.clone(),
            at: None,
            download: None,
        };

        let response: CatResponse = client.call(request).await?;

        let bytes = base64::engine::general_purpose::STANDARD.decode(&response.content)?;

        let content = match String::from_utf8(bytes.clone()) {
            Ok(text) => CatContent::Text(text),
            Err(_) => CatContent::Binary(bytes),
        };

        Ok(CatOutput {
            path: response.path,
            size: response.size,
            content,
        })
    }
}
