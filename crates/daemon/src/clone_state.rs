//! Hidden directory state management for cloned buckets
//!
//! This module manages the `.jax` directory that tracks the state of cloned buckets
//! on the local filesystem.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

use common::crypto::BLAKE3_HASH_SIZE;
use common::linked_data::{Hash, Link};

/// Name of the hidden directory used to track clone state
pub const CLONE_STATE_DIR: &str = ".jax";

/// Configuration stored in the .jax directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneConfig {
    /// The bucket ID being cloned
    pub bucket_id: Uuid,
    /// The bucket name
    pub bucket_name: String,
    /// The last synced manifest link
    pub last_synced_link: Link,
    /// The last synced height
    pub last_synced_height: u64,
}

/// Mapping of filesystem paths to their content hashes
/// This allows detecting local changes without full decryption
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PathHashMap {
    /// Map from relative path to (blob_hash, plaintext_hash)
    pub entries: HashMap<PathBuf, (Hash, [u8; BLAKE3_HASH_SIZE])>,
}

impl PathHashMap {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn insert(
        &mut self,
        path: PathBuf,
        blob_hash: Hash,
        plaintext_hash: [u8; BLAKE3_HASH_SIZE],
    ) {
        self.entries.insert(path, (blob_hash, plaintext_hash));
    }
}

/// Manages the hidden .jax directory state
pub struct CloneStateManager {
    /// Root directory of the clone (contains .jax directory)
    root_dir: PathBuf,
}

impl CloneStateManager {
    /// Create a new state manager for a clone directory
    pub fn new(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }

    /// Get the path to the .jax directory
    pub fn state_dir(&self) -> PathBuf {
        self.root_dir.join(CLONE_STATE_DIR)
    }

    /// Get the path to the config file
    fn config_path(&self) -> PathBuf {
        self.state_dir().join("config.json")
    }

    /// Get the path to the hash map file
    fn hash_map_path(&self) -> PathBuf {
        self.state_dir().join("hashes.json")
    }

    /// Initialize the .jax directory for a new clone
    pub fn init(&self, config: CloneConfig) -> Result<(), CloneStateError> {
        let state_dir = self.state_dir();

        // Create .jax directory
        std::fs::create_dir_all(&state_dir)?;

        // Write config
        self.write_config(&config)?;

        // Initialize empty hash map
        self.write_hash_map(&PathHashMap::new())?;

        Ok(())
    }

    /// Check if this directory has been initialized as a clone
    pub fn is_initialized(&self) -> bool {
        self.config_path().exists()
    }

    /// Read the clone configuration
    pub fn read_config(&self) -> Result<CloneConfig, CloneStateError> {
        let config_data = std::fs::read_to_string(self.config_path())?;
        let config: CloneConfig = serde_json::from_str(&config_data)?;
        Ok(config)
    }

    /// Write the clone configuration
    pub fn write_config(&self, config: &CloneConfig) -> Result<(), CloneStateError> {
        let config_json = serde_json::to_string_pretty(config)?;
        std::fs::write(self.config_path(), config_json)?;
        Ok(())
    }

    /// Write the path hash map
    pub fn write_hash_map(&self, hash_map: &PathHashMap) -> Result<(), CloneStateError> {
        let hash_map_json = serde_json::to_string_pretty(hash_map)?;
        std::fs::write(self.hash_map_path(), hash_map_json)?;
        Ok(())
    }

    /// Update the last synced state
    pub fn update_sync_state(&self, link: Link, height: u64) -> Result<(), CloneStateError> {
        let mut config = self.read_config()?;
        config.last_synced_link = link;
        config.last_synced_height = height;
        self.write_config(&config)?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CloneStateError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Clone directory not initialized (no .jax directory found)")]
    NotInitialized,
}
