//! Shared types for daemon module

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use common::crypto::BLAKE3_HASH_SIZE;
use common::linked_data::Hash;

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

    #[allow(dead_code)]
    pub fn get(&self, path: &Path) -> Option<&(Hash, [u8; BLAKE3_HASH_SIZE])> {
        self.entries.get(path)
    }

    #[allow(dead_code)]
    pub fn remove(&mut self, path: &Path) -> Option<(Hash, [u8; BLAKE3_HASH_SIZE])> {
        self.entries.remove(path)
    }
}
