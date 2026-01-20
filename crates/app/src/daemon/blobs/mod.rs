//! Blob storage module for the daemon.
//!
//! This module provides the blob store setup following the same pattern as the database module.

mod setup;

use std::path::Path;

use common::peer::BlobsStore;

use crate::state::BlobStoreConfig;

/// Wrapper around the legacy BlobsStore.
///
/// This provides a consistent interface for blob storage setup,
/// following the same pattern as the Database module.
#[derive(Clone)]
pub struct Blobs(BlobsStore);

impl Blobs {
    /// Setup blob storage based on configuration.
    ///
    /// # Arguments
    /// * `config` - Blob store configuration (Legacy, Filesystem, or S3)
    /// * `jax_dir` - Path to the jax directory (used for legacy blobs and cache)
    pub async fn setup(config: &BlobStoreConfig, jax_dir: &Path) -> Result<Self, BlobsSetupError> {
        let store = setup::setup_blobs_store(config, jax_dir).await?;
        Ok(Self(store))
    }

    /// Consume self and return the underlying BlobsStore.
    pub fn into_inner(self) -> BlobsStore {
        self.0
    }
}

/// Error type for blob store setup.
#[derive(Debug, thiserror::Error)]
pub enum BlobsSetupError {
    #[error("blob store error: {0}")]
    StoreError(String),
}
