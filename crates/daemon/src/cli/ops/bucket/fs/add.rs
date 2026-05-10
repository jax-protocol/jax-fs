use std::env;
use std::fmt;
use std::path::PathBuf;

use clap::Args;

use crate::cli::ui;
use jax_daemon::http_server::api::client::{resolve_bucket, ApiError};
use jax_daemon::http_server::api::v0::bucket::add::AddResponse;
use reqwest::multipart;

#[derive(Args, Debug, Clone)]
pub struct Add {
    /// Bucket name or UUID
    pub bucket: String,

    /// Path to file on filesystem
    pub path: String,

    /// Path in bucket where file should be mounted
    #[arg(long)]
    pub mount_path: String,
}

#[derive(Debug)]
pub struct AddOutput {
    pub successful: usize,
    pub failed: usize,
    pub bucket_link: String,
}

impl fmt::Display for AddOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.failed > 0 {
            writeln!(
                f,
                "{}",
                ui::success(
                    "Uploaded",
                    &format!("{} file(s), {} failed", self.successful, self.failed)
                )
            )?;
        } else {
            writeln!(
                f,
                "{}",
                ui::success("Uploaded", &format!("{} file(s)", self.successful))
            )?;
        }
        write!(f, "{}", ui::label("link", &self.bucket_link))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AddError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("HTTP error: {0}")]
    Reqwest(#[from] reqwest::Error),
}

#[async_trait::async_trait]
impl crate::cli::op::Op for Add {
    type Error = AddError;
    type Output = AddOutput;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();
        let bucket_id = resolve_bucket(&mut client, &self.bucket).await?;

        // Normalize path to absolute
        let path = PathBuf::from(&self.path);
        let absolute_path = if path.is_absolute() {
            path
        } else {
            env::current_dir()?.join(&path)
        };

        // Read the file
        let file_data = std::fs::read(&absolute_path)?;

        // Build multipart form
        let form = multipart::Form::new()
            .text("bucket_id", bucket_id.to_string())
            .text("mount_path", self.mount_path.clone())
            .part("file", multipart::Part::bytes(file_data));

        // Send multipart request
        let url = client.base_url().join("/api/v0/bucket/add").unwrap();
        let response = client
            .http_client()
            .post(url)
            .multipart(form)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            return Err(AddError::Api(ApiError::HttpStatus(status, body)));
        }

        let response: AddResponse = response.json().await?;

        Ok(AddOutput {
            successful: response.successful_files,
            failed: response.failed_files,
            bucket_link: response.bucket_link.hash().to_string(),
        })
    }
}
