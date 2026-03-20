use std::path::Path;
use std::time::Duration;

use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};
use sqlx::ConnectOptions;
use sqlx::Row;
use tracing::log::LevelFilter;

/// Layer 1: Path index backed by a dedicated SQLite database.
///
/// Maps (bucket_id, height, path, transform_params) → content_hash.
/// Avoids tree traversal and decryption on cache hit.
#[derive(Clone, Debug)]
pub struct CacheDatabase {
    pool: SqlitePool,
}

impl CacheDatabase {
    /// Open (or create) a cache database at the given path.
    pub async fn open(db_path: &Path) -> Result<Self, CacheDatabaseError> {
        let opts = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .log_statements(LevelFilter::Trace)
            .log_slow_statements(LevelFilter::Warn, Duration::from_millis(100));

        let pool = SqlitePoolOptions::new()
            .min_connections(1)
            .max_connections(4)
            .idle_timeout(Duration::from_secs(90))
            .connect_with(opts)
            .await
            .map_err(CacheDatabaseError::Connect)?;

        Self::init_schema(&pool).await?;
        Ok(Self { pool })
    }

    /// Create an in-memory cache database (for tests).
    pub async fn in_memory() -> Result<Self, CacheDatabaseError> {
        let opts = SqliteConnectOptions::new()
            .filename(":memory:")
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .log_statements(LevelFilter::Off);

        let pool = SqlitePoolOptions::new()
            .min_connections(1)
            .max_connections(1)
            .connect_with(opts)
            .await
            .map_err(CacheDatabaseError::Connect)?;

        Self::init_schema(&pool).await?;
        Ok(Self { pool })
    }

    async fn init_schema(pool: &SqlitePool) -> Result<(), CacheDatabaseError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS gateway_cache (
                bucket_id       TEXT    NOT NULL,
                height          INTEGER NOT NULL,
                path            TEXT    NOT NULL,
                transform_params TEXT   NOT NULL DEFAULT '',
                content_hash    TEXT    NOT NULL,
                content_size    INTEGER NOT NULL DEFAULT 0,
                mime_type       TEXT    NOT NULL DEFAULT 'application/octet-stream',
                created_at      INTEGER NOT NULL DEFAULT (unixepoch()),
                last_accessed   INTEGER NOT NULL DEFAULT (unixepoch()),
                PRIMARY KEY (bucket_id, height, path, transform_params)
            )",
        )
        .execute(pool)
        .await
        .map_err(CacheDatabaseError::Query)?;

        // Index for eviction by height per bucket
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_cache_bucket_height
             ON gateway_cache (bucket_id, height)",
        )
        .execute(pool)
        .await
        .map_err(CacheDatabaseError::Query)?;

        // Index for LRU eviction
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_cache_last_accessed
             ON gateway_cache (last_accessed)",
        )
        .execute(pool)
        .await
        .map_err(CacheDatabaseError::Query)?;

        Ok(())
    }

    /// Look up a cached content hash for the given key.
    pub async fn lookup(
        &self,
        bucket_id: &str,
        height: i64,
        path: &str,
        transform_params: &str,
    ) -> Result<Option<CacheEntry>, CacheDatabaseError> {
        let row = sqlx::query(
            "SELECT content_hash, mime_type FROM gateway_cache
             WHERE bucket_id = ? AND height = ? AND path = ? AND transform_params = ?",
        )
        .bind(bucket_id)
        .bind(height)
        .bind(path)
        .bind(transform_params)
        .fetch_optional(&self.pool)
        .await
        .map_err(CacheDatabaseError::Query)?;

        if let Some(row) = row {
            // Touch last_accessed
            let _ = sqlx::query(
                "UPDATE gateway_cache SET last_accessed = unixepoch()
                 WHERE bucket_id = ? AND height = ? AND path = ? AND transform_params = ?",
            )
            .bind(bucket_id)
            .bind(height)
            .bind(path)
            .bind(transform_params)
            .execute(&self.pool)
            .await;

            Ok(Some(CacheEntry {
                content_hash: row.get("content_hash"),
                mime_type: row.get("mime_type"),
            }))
        } else {
            Ok(None)
        }
    }

    /// Insert or replace a cache entry.
    pub async fn insert(&self, entry: &InsertEntry<'_>) -> Result<(), CacheDatabaseError> {
        let InsertEntry {
            bucket_id,
            height,
            path,
            transform_params,
            content_hash,
            content_size,
            mime_type,
        } = entry;
        sqlx::query(
            "INSERT OR REPLACE INTO gateway_cache
             (bucket_id, height, path, transform_params, content_hash, content_size, mime_type,
              created_at, last_accessed)
             VALUES (?, ?, ?, ?, ?, ?, ?, unixepoch(), unixepoch())",
        )
        .bind(bucket_id)
        .bind(height)
        .bind(path)
        .bind(transform_params)
        .bind(content_hash)
        .bind(content_size)
        .bind(mime_type)
        .execute(&self.pool)
        .await
        .map_err(CacheDatabaseError::Query)?;

        Ok(())
    }

    /// Remove entries for old heights, keeping only the most recent `keep_versions` per bucket.
    pub async fn evict_old_heights(&self, keep_versions: u32) -> Result<u64, CacheDatabaseError> {
        // For each bucket, find the Nth-highest height and delete anything below it
        let result = sqlx::query(
            "DELETE FROM gateway_cache
             WHERE rowid IN (
                 SELECT gc.rowid FROM gateway_cache gc
                 WHERE gc.height < (
                     SELECT COALESCE(MIN(h), 0) FROM (
                         SELECT DISTINCT height AS h FROM gateway_cache gc2
                         WHERE gc2.bucket_id = gc.bucket_id
                         ORDER BY height DESC
                         LIMIT ?
                     )
                 )
             )",
        )
        .bind(keep_versions)
        .execute(&self.pool)
        .await
        .map_err(CacheDatabaseError::Query)?;

        Ok(result.rows_affected())
    }

    /// Remove LRU entries until total_size is under the limit.
    /// Returns the hashes of removed entries.
    pub async fn evict_lru(&self, max_total_size: i64) -> Result<Vec<String>, CacheDatabaseError> {
        let total: i64 =
            sqlx::query_scalar("SELECT COALESCE(SUM(content_size), 0) FROM gateway_cache")
                .fetch_one(&self.pool)
                .await
                .map_err(CacheDatabaseError::Query)?;

        if total <= max_total_size {
            return Ok(Vec::new());
        }

        let to_free = total - max_total_size;
        let mut freed: i64 = 0;
        let mut removed_hashes = Vec::new();

        let rows: Vec<(i64, String, String, i64, String)> = sqlx::query_as(
            "SELECT rowid, bucket_id, path, height, content_hash FROM gateway_cache
             ORDER BY last_accessed ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(CacheDatabaseError::Query)?;

        for (rowid, _bucket_id, _path, _height, hash) in rows {
            if freed >= to_free {
                break;
            }

            let size: i64 =
                sqlx::query_scalar("SELECT content_size FROM gateway_cache WHERE rowid = ?")
                    .bind(rowid)
                    .fetch_one(&self.pool)
                    .await
                    .map_err(CacheDatabaseError::Query)?;

            sqlx::query("DELETE FROM gateway_cache WHERE rowid = ?")
                .bind(rowid)
                .execute(&self.pool)
                .await
                .map_err(CacheDatabaseError::Query)?;

            freed += size;
            removed_hashes.push(hash);
        }

        Ok(removed_hashes)
    }

    /// Get all content hashes still referenced in the index.
    pub async fn referenced_hashes(&self) -> Result<Vec<String>, CacheDatabaseError> {
        let hashes: Vec<(String,)> =
            sqlx::query_as("SELECT DISTINCT content_hash FROM gateway_cache")
                .fetch_all(&self.pool)
                .await
                .map_err(CacheDatabaseError::Query)?;

        Ok(hashes.into_iter().map(|(h,)| h).collect())
    }

    /// Remove entries older than `max_age` seconds.
    pub async fn evict_expired(
        &self,
        max_age_secs: i64,
    ) -> Result<Vec<String>, CacheDatabaseError> {
        let hashes: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT content_hash FROM gateway_cache
             WHERE created_at < unixepoch() - ?",
        )
        .bind(max_age_secs)
        .fetch_all(&self.pool)
        .await
        .map_err(CacheDatabaseError::Query)?;

        sqlx::query("DELETE FROM gateway_cache WHERE created_at < unixepoch() - ?")
            .bind(max_age_secs)
            .execute(&self.pool)
            .await
            .map_err(CacheDatabaseError::Query)?;

        Ok(hashes.into_iter().map(|(h,)| h).collect())
    }

    /// Count total entries (used in tests).
    #[cfg(test)]
    pub async fn count(&self) -> Result<i64, CacheDatabaseError> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM gateway_cache")
            .fetch_one(&self.pool)
            .await
            .map_err(CacheDatabaseError::Query)?;
        Ok(count)
    }

    /// Total cached content size (used in tests).
    #[cfg(test)]
    pub async fn total_size(&self) -> Result<i64, CacheDatabaseError> {
        let size: i64 =
            sqlx::query_scalar("SELECT COALESCE(SUM(content_size), 0) FROM gateway_cache")
                .fetch_one(&self.pool)
                .await
                .map_err(CacheDatabaseError::Query)?;
        Ok(size)
    }
}

/// Parameters for inserting a cache entry.
pub struct InsertEntry<'a> {
    pub bucket_id: &'a str,
    pub height: i64,
    pub path: &'a str,
    pub transform_params: &'a str,
    pub content_hash: &'a str,
    pub content_size: i64,
    pub mime_type: &'a str,
}

/// A single cache index entry returned by lookup.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub content_hash: String,
    pub mime_type: String,
}

#[derive(Debug, thiserror::Error)]
pub enum CacheDatabaseError {
    #[error("failed to connect to cache database: {0}")]
    Connect(sqlx::Error),
    #[error("cache database query error: {0}")]
    Query(sqlx::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry<'a>(
        bucket_id: &'a str,
        height: i64,
        path: &'a str,
        transform_params: &'a str,
        content_hash: &'a str,
        content_size: i64,
        mime_type: &'a str,
    ) -> InsertEntry<'a> {
        InsertEntry {
            bucket_id,
            height,
            path,
            transform_params,
            content_hash,
            content_size,
            mime_type,
        }
    }

    #[tokio::test]
    async fn test_insert_and_lookup() {
        let db = CacheDatabase::in_memory().await.unwrap();

        // Miss on empty database
        let result = db.lookup("bucket-1", 1, "/photo.jpg", "").await.unwrap();
        assert!(result.is_none());

        // Insert and hit
        db.insert(&entry(
            "bucket-1",
            1,
            "/photo.jpg",
            "",
            "abc123",
            1024,
            "image/jpeg",
        ))
        .await
        .unwrap();

        let e = db
            .lookup("bucket-1", 1, "/photo.jpg", "")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(e.content_hash, "abc123");
        assert_eq!(e.mime_type, "image/jpeg");
    }

    #[tokio::test]
    async fn test_transform_params_differentiate_entries() {
        let db = CacheDatabase::in_memory().await.unwrap();

        db.insert(&entry(
            "b1",
            1,
            "/photo.jpg",
            "",
            "hash-original",
            5000,
            "image/jpeg",
        ))
        .await
        .unwrap();
        db.insert(&entry(
            "b1",
            1,
            "/photo.jpg",
            "w=200",
            "hash-thumb",
            500,
            "image/jpeg",
        ))
        .await
        .unwrap();

        let original = db.lookup("b1", 1, "/photo.jpg", "").await.unwrap().unwrap();
        assert_eq!(original.content_hash, "hash-original");

        let thumb = db
            .lookup("b1", 1, "/photo.jpg", "w=200")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(thumb.content_hash, "hash-thumb");
    }

    #[tokio::test]
    async fn test_evict_old_heights() {
        let db = CacheDatabase::in_memory().await.unwrap();

        for h in 1..=3 {
            let hash = format!("hash-{}", h);
            db.insert(&entry("b1", h, "/file.txt", "", &hash, 100, "text/plain"))
                .await
                .unwrap();
        }
        assert_eq!(db.count().await.unwrap(), 3);

        // Keep only 1 version — should remove heights 1 and 2
        let removed = db.evict_old_heights(1).await.unwrap();
        assert_eq!(removed, 2);
        assert_eq!(db.count().await.unwrap(), 1);

        // Height 3 should remain
        assert!(db.lookup("b1", 3, "/file.txt", "").await.unwrap().is_some());
        assert!(db.lookup("b1", 1, "/file.txt", "").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_total_size_and_count() {
        let db = CacheDatabase::in_memory().await.unwrap();

        db.insert(&entry("b1", 1, "/a.txt", "", "h1", 100, "text/plain"))
            .await
            .unwrap();
        db.insert(&entry("b1", 1, "/b.txt", "", "h2", 200, "text/plain"))
            .await
            .unwrap();

        assert_eq!(db.count().await.unwrap(), 2);
        assert_eq!(db.total_size().await.unwrap(), 300);
    }
}
