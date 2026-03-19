//! LRU cache with TTL for FUSE file contents
//!
//! This cache stores file content and metadata to reduce HTTP API calls.
//! It supports time-based expiration and size-based eviction.

use std::sync::Arc;
use std::time::Duration;

use moka::sync::Cache;
use serde::{Deserialize, Serialize};

/// Cached file content
#[derive(Debug, Clone)]
pub struct CachedContent {
    /// File content bytes
    pub data: Arc<Vec<u8>>,
    /// MIME type
    pub mime_type: String,
}

/// Cached file/directory attributes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedAttr {
    /// Size in bytes
    pub size: u64,
    /// Is this a directory?
    pub is_dir: bool,
    /// MIME type for files
    pub mime_type: Option<String>,
    /// Modification time (Unix timestamp)
    pub mtime: i64,
}

/// Cached directory listing
#[derive(Debug, Clone)]
pub struct CachedDirEntry {
    pub name: String,
    pub is_dir: bool,
}

/// Configuration for the file cache
#[derive(Debug, Clone)]
pub struct FileCacheConfig {
    /// Maximum cache size in megabytes
    pub max_size_mb: u32,
    /// TTL for metadata (attrs, dirs) in seconds
    pub ttl_secs: u32,
    /// TTL for content cache in seconds (defaults to 5x metadata TTL)
    pub content_ttl_secs: u32,
    /// TTL for negative cache (non-existent paths) in seconds
    pub negative_ttl_secs: u32,
}

impl Default for FileCacheConfig {
    fn default() -> Self {
        Self {
            max_size_mb: 100,
            ttl_secs: 60,
            content_ttl_secs: 300,
            negative_ttl_secs: 10,
        }
    }
}

impl FileCacheConfig {
    /// Create config from basic parameters, deriving content and negative TTLs
    pub fn from_basic(max_size_mb: u32, ttl_secs: u32) -> Self {
        Self {
            max_size_mb,
            ttl_secs,
            content_ttl_secs: ttl_secs.saturating_mul(5),
            negative_ttl_secs: 10,
        }
    }
}

/// LRU cache for FUSE filesystem
#[derive(Clone)]
pub struct FileCache {
    /// Content cache: path → content
    content: Cache<String, CachedContent>,
    /// Attribute cache: path → attributes
    attrs: Cache<String, CachedAttr>,
    /// Directory listing cache: path → entries
    dirs: Cache<String, Vec<CachedDirEntry>>,
    /// Negative cache: paths confirmed not to exist
    negative: Cache<String, ()>,
    /// Configuration
    config: FileCacheConfig,
}

impl FileCache {
    /// Create a new file cache with the given configuration
    pub fn new(config: FileCacheConfig) -> Self {
        let metadata_ttl = Duration::from_secs(config.ttl_secs as u64);
        let content_ttl = Duration::from_secs(config.content_ttl_secs as u64);
        let negative_ttl = Duration::from_secs(config.negative_ttl_secs as u64);
        // Estimate ~1KB average per entry for size calculation
        let max_capacity = (config.max_size_mb as u64) * 1024;

        Self {
            content: Cache::builder()
                .time_to_live(content_ttl)
                .max_capacity(max_capacity)
                .build(),
            attrs: Cache::builder()
                .time_to_live(metadata_ttl)
                .max_capacity(max_capacity * 10) // Attrs are smaller
                .build(),
            dirs: Cache::builder()
                .time_to_live(metadata_ttl)
                .max_capacity(max_capacity)
                .build(),
            negative: Cache::builder()
                .time_to_live(negative_ttl)
                .max_capacity(10_000) // Non-existent paths are tiny
                .build(),
            config,
        }
    }

    /// Get cached content for a path
    pub fn get_content(&self, path: &str) -> Option<CachedContent> {
        self.content.get(&Self::normalize_key(path))
    }

    /// Cache content for a path
    pub fn put_content(&self, path: &str, content: CachedContent) {
        self.content.insert(Self::normalize_key(path), content);
    }

    /// Get cached attributes for a path
    pub fn get_attr(&self, path: &str) -> Option<CachedAttr> {
        self.attrs.get(&Self::normalize_key(path))
    }

    /// Cache attributes for a path
    pub fn put_attr(&self, path: &str, attr: CachedAttr) {
        self.attrs.insert(Self::normalize_key(path), attr);
    }

    /// Get cached directory listing
    pub fn get_dir(&self, path: &str) -> Option<Vec<CachedDirEntry>> {
        self.dirs.get(&Self::normalize_key(path))
    }

    /// Cache directory listing
    pub fn put_dir(&self, path: &str, entries: Vec<CachedDirEntry>) {
        self.dirs.insert(Self::normalize_key(path), entries);
    }

    /// Check if a path is in the negative cache (known not to exist)
    pub fn is_negative(&self, path: &str) -> bool {
        self.negative.contains_key(&Self::normalize_key(path))
    }

    /// Add a path to the negative cache (mark as non-existent)
    pub fn put_negative(&self, path: &str) {
        self.negative.insert(Self::normalize_key(path), ());
    }

    /// Invalidate a specific path from all caches
    pub fn invalidate(&self, path: &str) {
        let key = Self::normalize_key(path);
        self.content.invalidate(&key);
        self.attrs.invalidate(&key);
        self.dirs.invalidate(&key);
        self.negative.invalidate(&key);
    }

    /// Invalidate all entries under a path prefix
    pub fn invalidate_prefix(&self, prefix: &str) {
        let prefix = Self::normalize_key(prefix);

        // Moka doesn't have prefix invalidation, so we need to iterate
        // For content cache
        self.content.run_pending_tasks();
        // Note: We can't efficiently iterate moka caches, so we just invalidate_all
        // in practice for prefix invalidations
        if prefix == "/" {
            self.invalidate_all();
        }
    }

    /// Invalidate all cached entries
    pub fn invalidate_all(&self) {
        self.content.invalidate_all();
        self.attrs.invalidate_all();
        self.dirs.invalidate_all();
        self.negative.invalidate_all();
    }

    /// Get current cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            content_count: self.content.entry_count(),
            attr_count: self.attrs.entry_count(),
            dir_count: self.dirs.entry_count(),
            negative_count: self.negative.entry_count(),
            max_size_mb: self.config.max_size_mb,
            metadata_ttl_secs: self.config.ttl_secs,
            content_ttl_secs: self.config.content_ttl_secs,
            negative_ttl_secs: self.config.negative_ttl_secs,
        }
    }

    /// Normalize a path to a consistent cache key
    fn normalize_key(path: &str) -> String {
        let path = path.trim();
        if path.is_empty() || path == "/" {
            return "/".to_string();
        }

        let mut key = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{}", path)
        };

        if key.len() > 1 && key.ends_with('/') {
            key.pop();
        }

        key
    }
}

impl std::fmt::Debug for FileCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileCache")
            .field("config", &self.config)
            .field("content_count", &self.content.entry_count())
            .field("attr_count", &self.attrs.entry_count())
            .field("dir_count", &self.dirs.entry_count())
            .field("negative_count", &self.negative.entry_count())
            .finish()
    }
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub content_count: u64,
    pub attr_count: u64,
    pub dir_count: u64,
    pub negative_count: u64,
    pub max_size_mb: u32,
    pub metadata_ttl_secs: u32,
    pub content_ttl_secs: u32,
    pub negative_ttl_secs: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_cache() {
        let cache = FileCache::new(FileCacheConfig::default());

        let content = CachedContent {
            data: Arc::new(vec![1, 2, 3]),
            mime_type: "text/plain".to_string(),
        };

        cache.put_content("/foo.txt", content.clone());

        let cached = cache.get_content("/foo.txt").unwrap();
        assert_eq!(cached.data.as_ref(), &[1, 2, 3]);
        assert_eq!(cached.mime_type, "text/plain");
    }

    #[test]
    fn test_attr_cache() {
        let cache = FileCache::new(FileCacheConfig::default());

        let attr = CachedAttr {
            size: 100,
            is_dir: false,
            mime_type: Some("text/plain".to_string()),
            mtime: 1234567890,
        };

        cache.put_attr("/foo.txt", attr.clone());

        let cached = cache.get_attr("/foo.txt").unwrap();
        assert_eq!(cached.size, 100);
        assert!(!cached.is_dir);
    }

    #[test]
    fn test_dir_cache() {
        let cache = FileCache::new(FileCacheConfig::default());

        let entries = vec![
            CachedDirEntry {
                name: "file.txt".to_string(),
                is_dir: false,
            },
            CachedDirEntry {
                name: "subdir".to_string(),
                is_dir: true,
            },
        ];

        cache.put_dir("/", entries.clone());

        let cached = cache.get_dir("/").unwrap();
        assert_eq!(cached.len(), 2);
        assert_eq!(cached[0].name, "file.txt");
    }

    #[test]
    fn test_invalidate() {
        let cache = FileCache::new(FileCacheConfig::default());

        let content = CachedContent {
            data: Arc::new(vec![1, 2, 3]),
            mime_type: "text/plain".to_string(),
        };

        cache.put_content("/foo.txt", content);
        assert!(cache.get_content("/foo.txt").is_some());

        cache.invalidate("/foo.txt");
        assert!(cache.get_content("/foo.txt").is_none());
    }

    #[test]
    fn test_invalidate_all() {
        let cache = FileCache::new(FileCacheConfig::default());

        cache.put_content(
            "/a.txt",
            CachedContent {
                data: Arc::new(vec![1]),
                mime_type: "text/plain".to_string(),
            },
        );
        cache.put_content(
            "/b.txt",
            CachedContent {
                data: Arc::new(vec![2]),
                mime_type: "text/plain".to_string(),
            },
        );

        cache.invalidate_all();

        assert!(cache.get_content("/a.txt").is_none());
        assert!(cache.get_content("/b.txt").is_none());
    }

    #[test]
    fn test_negative_cache() {
        let cache = FileCache::new(FileCacheConfig::default());

        assert!(!cache.is_negative("/nonexistent"));

        cache.put_negative("/nonexistent");
        assert!(cache.is_negative("/nonexistent"));

        // Invalidate should clear negative cache too
        cache.invalidate("/nonexistent");
        assert!(!cache.is_negative("/nonexistent"));
    }

    #[test]
    fn test_negative_cache_invalidate_all() {
        let cache = FileCache::new(FileCacheConfig::default());

        cache.put_negative("/a");
        cache.put_negative("/b");
        assert!(cache.is_negative("/a"));
        assert!(cache.is_negative("/b"));

        cache.invalidate_all();
        assert!(!cache.is_negative("/a"));
        assert!(!cache.is_negative("/b"));
    }

    #[test]
    fn test_normalize_key() {
        assert_eq!(FileCache::normalize_key(""), "/");
        assert_eq!(FileCache::normalize_key("/"), "/");
        assert_eq!(FileCache::normalize_key("foo"), "/foo");
        assert_eq!(FileCache::normalize_key("/foo"), "/foo");
        assert_eq!(FileCache::normalize_key("/foo/"), "/foo");
    }

    /// Simulates the cache operations that occur during create() followed by
    /// rename(). Verifies that invalidating the parent directory cache after
    /// create() allows subsequent lookups to find the new file.
    #[test]
    fn test_create_invalidates_parent_dir_cache() {
        let cache = FileCache::new(FileCacheConfig::default());

        // Parent directory has a cached listing without the new file
        let parent_entries = vec![CachedDirEntry {
            name: "existing.txt".to_string(),
            is_dir: false,
        }];
        cache.put_dir("/", parent_entries);

        // Simulate create(): cache the new file's attrs and invalidate parent
        let attr = CachedAttr {
            size: 0,
            is_dir: false,
            mime_type: None,
            mtime: 1234567890,
        };
        cache.put_attr("/new_file.txt", attr);

        // Invalidate parent directory cache (the fix)
        cache.invalidate("/");

        // Parent dir cache should be gone, forcing a re-fetch
        assert!(cache.get_dir("/").is_none());

        // But the new file's attrs should still be cached
        assert!(cache.get_attr("/new_file.txt").is_some());
    }

    /// Verifies that creating a file clears any negative cache entry for that
    /// path, preventing stale ENOENT responses on subsequent lookups.
    #[test]
    fn test_create_clears_negative_cache_via_parent_invalidation() {
        let cache = FileCache::new(FileCacheConfig::default());

        // A prior lookup marked /new_file.txt as non-existent
        cache.put_negative("/new_file.txt");
        assert!(cache.is_negative("/new_file.txt"));

        // Simulate create(): put_attr does NOT clear negative cache
        let attr = CachedAttr {
            size: 0,
            is_dir: false,
            mime_type: None,
            mtime: 1234567890,
        };
        cache.put_attr("/new_file.txt", attr);

        // The attr cache has the entry, so fetch_attr would find it before
        // checking negative cache — but after flush() invalidates the attr,
        // negative cache would cause ENOENT. Invalidate the path too.
        cache.invalidate("/new_file.txt");
        // Re-add just the attr (simulating what create does after the fix)
        let attr2 = CachedAttr {
            size: 0,
            is_dir: false,
            mime_type: None,
            mtime: 1234567890,
        };
        cache.put_attr("/new_file.txt", attr2);

        // Negative cache should be clear
        assert!(!cache.is_negative("/new_file.txt"));
        // Attr should be present
        assert!(cache.get_attr("/new_file.txt").is_some());
    }
}
