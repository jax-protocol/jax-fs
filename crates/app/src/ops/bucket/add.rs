use crate::daemon::http_server::api::client::ApiError;
use crate::daemon::http_server::api::v0::bucket::add::AddResponse;
use clap::Args;
use reqwest::multipart;
use std::env;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Args, Debug, Clone)]
pub struct Add {
    /// Bucket ID (or use --name)
    #[arg(long, group = "bucket_identifier")]
    pub bucket_id: Option<Uuid>,

    /// Bucket name (or use --bucket-id)
    #[arg(long, group = "bucket_identifier")]
    pub name: Option<String>,

    /// Absolute path to file on filesystem
    #[arg(long)]
    pub path: String,

    /// Path in bucket where file should be mounted
    #[arg(long)]
    pub mount_path: String,
}

#[derive(Debug, thiserror::Error)]
pub enum BucketAddError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("HTTP error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Either --bucket-id or --name must be provided")]
    NoBucketIdentifier,
}

#[async_trait::async_trait]
impl crate::op::Op for Add {
    type Error = BucketAddError;
    type Output = String;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();

        // Resolve bucket name to UUID if needed
        let bucket_id = if let Some(id) = self.bucket_id {
            id
        } else if let Some(ref name) = self.name {
            client.resolve_bucket_name(name).await?
        } else {
            return Err(BucketAddError::NoBucketIdentifier);
        };

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
            return Err(BucketAddError::Api(ApiError::HttpStatus(status, body)));
        }

        let response: AddResponse = response.json().await?;

        if response.failed_files > 0 {
            Ok(format!(
                "Uploaded {} file(s) successfully, {} failed (bucket link: {})",
                response.successful_files,
                response.failed_files,
                response.bucket_link.hash()
            ))
        } else {
            Ok(format!(
                "Uploaded {} file(s) successfully (bucket link: {})",
                response.successful_files,
                response.bucket_link.hash()
            ))
        }
    }
}
