use clap::Args;
use std::path::PathBuf;
use uuid::Uuid;

use crate::daemon::http_server::api::client::ApiError;
use crate::op::Op;

use super::clone_state::{CloneConfig, CloneStateError, CloneStateManager, PathHashMap};

#[derive(Args, Debug, Clone)]
pub struct Clone {
    /// Bucket ID (or use --name)
    #[arg(long, group = "bucket_identifier")]
    pub bucket_id: Option<Uuid>,

    /// Bucket name (or use --bucket-id)
    #[arg(long, group = "bucket_identifier")]
    pub name: Option<String>,

    /// Directory to clone into (will be created if it doesn't exist)
    #[arg(long)]
    pub directory: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum CloneError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Clone state error: {0}")]
    CloneState(#[from] CloneStateError),
    #[error("Either --bucket-id or --name must be provided")]
    NoBucketIdentifier,
    #[error("Directory already exists and is not empty: {0}")]
    DirectoryNotEmpty(PathBuf),
    #[error("Directory already initialized as a clone")]
    AlreadyCloned,
    #[error("HTTP error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),
}

#[async_trait::async_trait]
impl Op for Clone {
    type Error = CloneError;
    type Output = String;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();

        // Resolve bucket name to UUID if needed
        let bucket_id = if let Some(id) = self.bucket_id {
            id
        } else if let Some(ref name) = self.name {
            client.resolve_bucket_name(name).await?
        } else {
            return Err(CloneError::NoBucketIdentifier);
        };

        // Check if directory exists and is empty
        if self.directory.exists() {
            let state_manager = CloneStateManager::new(self.directory.clone());
            if state_manager.is_initialized() {
                return Err(CloneError::AlreadyCloned);
            }

            // Check if directory is empty (except for .jax which we'll overwrite)
            let entries: Vec<_> = std::fs::read_dir(&self.directory)?
                .filter_map(|e| e.ok())
                .filter(|e| e.file_name() != ".jax")
                .collect();

            if !entries.is_empty() {
                return Err(CloneError::DirectoryNotEmpty(self.directory.clone()));
            }
        } else {
            // Create the directory
            std::fs::create_dir_all(&self.directory)?;
        }

        // Call the daemon's export endpoint
        // The daemon will use its local peer to export the bucket
        #[derive(serde::Serialize)]
        struct ExportRequest {
            bucket_id: Uuid,
            target_dir: PathBuf,
        }

        let export_request = ExportRequest {
            bucket_id,
            target_dir: self.directory.clone(),
        };

        #[derive(serde::Deserialize)]
        struct ExportResponse {
            bucket_name: String,
            link: common::linked_data::Link,
            height: u64,
            files_exported: usize,
            hash_map: PathHashMap,
        }

        let export_result: ExportResponse = client
            .http_client()
            .post(client.base_url().join("/api/v0/bucket/export").unwrap())
            .json(&export_request)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        // Initialize .jax directory with clone state
        let state_manager = CloneStateManager::new(self.directory.clone());
        let config = CloneConfig {
            bucket_id,
            bucket_name: export_result.bucket_name.clone(),
            last_synced_link: export_result.link,
            last_synced_height: export_result.height,
        };

        state_manager.init(config)?;
        state_manager.write_hash_map(&export_result.hash_map)?;

        Ok(format!(
            "Cloned bucket '{}' ({}) to {}\nExported {} files",
            export_result.bucket_name,
            bucket_id,
            self.directory.display(),
            export_result.files_exported
        ))
    }
}
