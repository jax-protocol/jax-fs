use std::path::{Path, PathBuf};

use bytes::Bytes;

/// Layer 2: Content-addressed blob store for cached decrypted/transformed content.
///
/// Stores blobs keyed by their BLAKE3 hash on the local filesystem.
/// Content is naturally deduplicated: the same plaintext at different paths
/// or across versions shares one blob.
#[derive(Clone, Debug)]
pub struct CacheStore {
    inner: CacheStoreInner,
}

#[derive(Clone, Debug)]
enum CacheStoreInner {
    Filesystem { base_path: PathBuf },
    Memory(std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, Bytes>>>),
}

impl CacheStore {
    /// Create a filesystem-backed cache store.
    pub async fn new_local(base_path: &Path) -> Result<Self, CacheStoreError> {
        tokio::fs::create_dir_all(base_path)
            .await
            .map_err(CacheStoreError::Io)?;
        Ok(Self {
            inner: CacheStoreInner::Filesystem {
                base_path: base_path.to_path_buf(),
            },
        })
    }

    /// Create an in-memory cache store (for tests).
    pub fn new_memory() -> Self {
        Self {
            inner: CacheStoreInner::Memory(Default::default()),
        }
    }

    /// Compute BLAKE3 hash of data and store it. Returns the hex hash.
    pub async fn put(&self, data: &[u8]) -> Result<String, CacheStoreError> {
        let hash = blake3::hash(data).to_hex().to_string();

        match &self.inner {
            CacheStoreInner::Filesystem { base_path } => {
                let blob_path = blob_path(base_path, &hash);
                if !blob_path.exists() {
                    if let Some(parent) = blob_path.parent() {
                        tokio::fs::create_dir_all(parent)
                            .await
                            .map_err(CacheStoreError::Io)?;
                    }
                    tokio::fs::write(&blob_path, data)
                        .await
                        .map_err(CacheStoreError::Io)?;
                }
            }
            CacheStoreInner::Memory(map) => {
                map.write()
                    .await
                    .insert(hash.clone(), Bytes::copy_from_slice(data));
            }
        }

        Ok(hash)
    }

    /// Retrieve a blob by its BLAKE3 hash.
    pub async fn get(&self, hash: &str) -> Result<Option<Bytes>, CacheStoreError> {
        match &self.inner {
            CacheStoreInner::Filesystem { base_path } => {
                let blob_path = blob_path(base_path, hash);
                match tokio::fs::read(&blob_path).await {
                    Ok(data) => Ok(Some(Bytes::from(data))),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                    Err(e) => Err(CacheStoreError::Io(e)),
                }
            }
            CacheStoreInner::Memory(map) => Ok(map.read().await.get(hash).cloned()),
        }
    }

    /// Delete a blob by hash. Returns true if it existed.
    pub async fn delete(&self, hash: &str) -> Result<bool, CacheStoreError> {
        match &self.inner {
            CacheStoreInner::Filesystem { base_path } => {
                let blob_path = blob_path(base_path, hash);
                match tokio::fs::remove_file(&blob_path).await {
                    Ok(()) => Ok(true),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
                    Err(e) => Err(CacheStoreError::Io(e)),
                }
            }
            CacheStoreInner::Memory(map) => Ok(map.write().await.remove(hash).is_some()),
        }
    }

    /// List all stored hashes.
    pub async fn list_hashes(&self) -> Result<Vec<String>, CacheStoreError> {
        match &self.inner {
            CacheStoreInner::Filesystem { base_path } => {
                let mut hashes = Vec::new();
                let mut dirs = tokio::fs::read_dir(base_path)
                    .await
                    .map_err(CacheStoreError::Io)?;
                while let Some(prefix_entry) =
                    dirs.next_entry().await.map_err(CacheStoreError::Io)?
                {
                    if prefix_entry.file_type().await.is_ok_and(|t| t.is_dir()) {
                        let mut files = tokio::fs::read_dir(prefix_entry.path())
                            .await
                            .map_err(CacheStoreError::Io)?;
                        while let Some(file_entry) =
                            files.next_entry().await.map_err(CacheStoreError::Io)?
                        {
                            if let Some(name) = file_entry.file_name().to_str() {
                                hashes.push(name.to_string());
                            }
                        }
                    }
                }
                Ok(hashes)
            }
            CacheStoreInner::Memory(map) => Ok(map.read().await.keys().cloned().collect()),
        }
    }
}

/// Path layout: {base}/{hash[0..2]}/{hash}
fn blob_path(base: &Path, hash: &str) -> PathBuf {
    let prefix = &hash[..2.min(hash.len())];
    base.join(prefix).join(hash)
}

#[derive(Debug, thiserror::Error)]
pub enum CacheStoreError {
    #[error("cache store I/O error: {0}")]
    Io(std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_put_and_get_memory() {
        let store = CacheStore::new_memory();
        let data = b"hello world";

        let hash = store.put(data).await.unwrap();
        assert!(!hash.is_empty());

        let retrieved = store.get(&hash).await.unwrap().unwrap();
        assert_eq!(retrieved.as_ref(), data);
    }

    #[tokio::test]
    async fn test_content_addressing() {
        let store = CacheStore::new_memory();

        let hash1 = store.put(b"same data").await.unwrap();
        let hash2 = store.put(b"same data").await.unwrap();
        assert_eq!(hash1, hash2, "same content should produce same hash");

        let hash3 = store.put(b"different data").await.unwrap();
        assert_ne!(
            hash1, hash3,
            "different content should produce different hash"
        );
    }

    #[tokio::test]
    async fn test_delete() {
        let store = CacheStore::new_memory();
        let hash = store.put(b"to delete").await.unwrap();

        assert!(store.delete(&hash).await.unwrap());
        assert!(!store.delete(&hash).await.unwrap());
        assert!(store.get(&hash).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let store = CacheStore::new_memory();
        assert!(store.get("nonexistent").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_filesystem_store() {
        let tmp = tempfile::tempdir().unwrap();
        let store = CacheStore::new_local(tmp.path()).await.unwrap();

        let data = b"filesystem test";
        let hash = store.put(data).await.unwrap();

        let retrieved = store.get(&hash).await.unwrap().unwrap();
        assert_eq!(retrieved.as_ref(), data);

        assert!(store.delete(&hash).await.unwrap());
        assert!(store.get(&hash).await.unwrap().is_none());
    }
}
