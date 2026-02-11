use clap::Args;
use std::path::PathBuf;

use crate::cli::op::Op;
use jax_daemon::http_server::api::client::ApiError;

use super::clone_state::{CloneStateError, CloneStateManager, PathHashMap};

#[derive(Args, Debug, Clone)]
pub struct Sync {
    /// Directory containing the cloned bucket (must have .jax directory)
    #[arg(long)]
    pub directory: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Clone state error: {0}")]
    CloneState(#[from] CloneStateError),
    #[error("Directory is not a cloned bucket (no .jax directory found)")]
    NotCloned,
    #[error("HTTP error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),
}

#[async_trait::async_trait]
impl Op for Sync {
    type Error = SyncError;
    type Output = String;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        let client = ctx.client.clone();

        // Check that directory is initialized as a clone
        let state_manager = CloneStateManager::new(self.directory.clone());
        if !state_manager.is_initialized() {
            return Err(SyncError::NotCloned);
        }

        // Read current clone config
        let old_config = state_manager.read_config()?;

        // Call the daemon's export endpoint
        // The daemon will use its local peer to export the current bucket state
        #[derive(serde::Serialize)]
        struct ExportRequest {
            bucket_id: uuid::Uuid,
            target_dir: PathBuf,
        }

        let export_request = ExportRequest {
            bucket_id: old_config.bucket_id,
            target_dir: self.directory.clone(),
        };

        #[derive(serde::Deserialize)]
        struct ExportResponse {
            #[allow(dead_code)]
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

        // Check if there were updates
        if export_result.height == old_config.last_synced_height
            && export_result.link == old_config.last_synced_link
        {
            return Ok(format!(
                "Bucket '{}' is already up to date (height: {})",
                old_config.bucket_name, old_config.last_synced_height
            ));
        }

        // Update .jax state
        state_manager.update_sync_state(export_result.link.clone(), export_result.height)?;
        state_manager.write_hash_map(&export_result.hash_map)?;

        Ok(format!(
            "Synced bucket '{}' from height {} to height {}\nUpdated {} files",
            old_config.bucket_name,
            old_config.last_synced_height,
            export_result.height,
            export_result.files_exported
        ))
    }
}
