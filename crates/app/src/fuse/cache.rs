//! LRU cache for FUSE file content
//!
//! Uses moka for efficient concurrent caching with TTL and size-based eviction.

use moka::sync::Cache;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Configuration for the file cache
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum cache size in bytes (default: 100MB)
    pub max_bytes: u64,
    /// Time-to-live for cache entries in seconds (default: 60)
    pub ttl_secs: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_bytes: 100 * 1024 * 1024, // 100MB
            ttl_secs: 60,
        }
    }
}

/// LRU cache for file content with version-based invalidation
pub struct FileCache {
    /// Content cache: path -> (data, version at insert time)
    content: Cache<PathBuf, (Vec<u8>, u64)>,

    /// Current manifest version (incremented on remote sync or local write)
    version: AtomicU64,
}

impl FileCache {
    /// Create a new file cache with the given configuration
    pub fn new(config: CacheConfig) -> Self {
        // We use entry count as a proxy for size since moka doesn't have byte-based eviction
        // Assuming average file size of 10KB, 100MB / 10KB = 10,000 entries
        let max_entries = config.max_bytes / (10 * 1024);

        Self {
            content: Cache::builder()
                .max_capacity(max_entries)
                .time_to_live(Duration::from_secs(config.ttl_secs))
                .build(),
            version: AtomicU64::new(0),
        }
    }

    /// Get cached content for a path, if still valid
    pub fn get(&self, path: &Path) -> Option<Vec<u8>> {
        let current_version = self.version.load(Ordering::Relaxed);
        self.content
            .get(&path.to_path_buf())
            .filter(|(_, cached_version)| *cached_version == current_version)
            .map(|(data, _)| data)
    }

    /// Cache content for a path at the current version
    pub fn put(&self, path: PathBuf, data: Vec<u8>) {
        let version = self.version.load(Ordering::Relaxed);
        self.content.insert(path, (data, version));
    }

    /// Invalidate cache for a specific path
    pub fn invalidate(&self, path: &Path) {
        self.content.invalidate(&path.to_path_buf());
    }

    /// Invalidate a path and all its descendants (for directory operations)
    pub fn invalidate_prefix(&self, prefix: &Path) {
        // Bump version to invalidate everything - this is simpler than
        // iterating through all entries. Specific paths can be re-cached
        // on next access.
        self.invalidate_all();
    }

    /// Invalidate all cached content (e.g., on remote sync)
    pub fn invalidate_all(&self) {
        self.version.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current cache version
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Relaxed)
    }

    /// Get cache statistics for debugging
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entry_count: self.content.entry_count(),
            version: self.version.load(Ordering::Relaxed),
        }
    }
}

/// Cache statistics for debugging
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entry_count: u64,
    pub version: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_basic() {
        let cache = FileCache::new(CacheConfig::default());

        let path = PathBuf::from("/test/file.txt");
        let data = b"hello world".to_vec();

        // Initially empty
        assert!(cache.get(&path).is_none());

        // Put and get
        cache.put(path.clone(), data.clone());
        assert_eq!(cache.get(&path), Some(data.clone()));

        // Invalidate specific path
        cache.invalidate(&path);
        assert!(cache.get(&path).is_none());
    }

    #[test]
    fn test_cache_version_invalidation() {
        let cache = FileCache::new(CacheConfig::default());

        let path = PathBuf::from("/test/file.txt");
        let data = b"hello world".to_vec();

        cache.put(path.clone(), data.clone());
        assert_eq!(cache.get(&path), Some(data));

        // Invalidate all by bumping version
        cache.invalidate_all();
        assert!(cache.get(&path).is_none());
    }
}
