//! SQLite database for blob metadata storage.

use std::path::Path;

use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions},
    Row,
};

use crate::error::Result;

/// Blob metadata stored in SQLite.
///
/// All fields are populated from the database row; some are only
/// accessed in tests but are part of the schema mapping.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct BlobMetadata {
    pub hash: String,
    pub size: i64,
    pub has_outboard: bool,
    pub state: BlobState,
    pub created_at: i64,
    pub updated_at: i64,
}

/// State of a blob in the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum BlobState {
    /// Blob is complete and ready to use
    #[default]
    Complete,
    /// Blob upload is in progress
    Partial,
    /// Blob is marked for deletion
    Deleting,
}

impl BlobState {
    pub fn as_str(&self) -> &'static str {
        match self {
            BlobState::Complete => "complete",
            BlobState::Partial => "partial",
            BlobState::Deleting => "deleting",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "complete" => BlobState::Complete,
            "partial" => BlobState::Partial,
            "deleting" => BlobState::Deleting,
            _ => BlobState::Complete,
        }
    }
}

/// SQLite database connection pool.
#[derive(Debug, Clone)]
pub(crate) struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Create a new database connection from a file path.
    pub async fn new(path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        let db = Self { pool };
        db.run_migrations().await?;
        Ok(db)
    }

    /// Create an in-memory database.
    pub async fn in_memory() -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .journal_mode(SqliteJournalMode::Wal);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;

        let db = Self { pool };
        db.run_migrations().await?;
        Ok(db)
    }

    /// Run database migrations.
    async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }

    /// Insert a new blob record.
    pub async fn insert_blob(&self, hash: &str, size: i64, has_outboard: bool) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO blobs (hash, size, has_outboard, state, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(hash) DO UPDATE SET
                size = excluded.size,
                has_outboard = excluded.has_outboard,
                state = excluded.state,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(hash)
        .bind(size)
        .bind(has_outboard)
        .bind(BlobState::Complete.as_str())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get blob metadata by hash.
    pub async fn get_blob(&self, hash: &str) -> Result<Option<BlobMetadata>> {
        let row = sqlx::query(
            r#"
            SELECT hash, size, has_outboard, state, created_at, updated_at
            FROM blobs
            WHERE hash = ?
            "#,
        )
        .bind(hash)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| BlobMetadata {
            hash: r.get("hash"),
            size: r.get("size"),
            has_outboard: r.get::<i32, _>("has_outboard") != 0,
            state: BlobState::parse(r.get("state")),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }))
    }

    /// Check if a blob exists.
    pub async fn has_blob(&self, hash: &str) -> Result<bool> {
        let row = sqlx::query(
            r#"
            SELECT 1 FROM blobs WHERE hash = ? AND state = ?
            "#,
        )
        .bind(hash)
        .bind(BlobState::Complete.as_str())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.is_some())
    }

    /// Delete a blob record.
    pub async fn delete_blob(&self, hash: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM blobs WHERE hash = ?
            "#,
        )
        .bind(hash)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List all blob hashes.
    pub async fn list_blobs(&self) -> Result<Vec<String>> {
        let rows = sqlx::query(
            r#"
            SELECT hash FROM blobs WHERE state = ?
            ORDER BY created_at DESC
            "#,
        )
        .bind(BlobState::Complete.as_str())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| r.get("hash")).collect())
    }
}

#[cfg(test)]
impl Database {
    /// Count blobs.
    pub async fn count_blobs(&self) -> Result<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count FROM blobs WHERE state = ?
            "#,
        )
        .bind(BlobState::Complete.as_str())
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get("count"))
    }

    /// Get total size of all blobs.
    pub async fn total_size(&self) -> Result<i64> {
        let row = sqlx::query(
            r#"
            SELECT COALESCE(SUM(size), 0) as total FROM blobs WHERE state = ?
            "#,
        )
        .bind(BlobState::Complete.as_str())
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get("total"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_database() {
        let db = Database::in_memory().await.unwrap();

        // Insert a blob
        db.insert_blob("abc123", 1024, false).await.unwrap();

        // Verify it exists
        assert!(db.has_blob("abc123").await.unwrap());

        // Get metadata
        let meta = db.get_blob("abc123").await.unwrap().unwrap();
        assert_eq!(meta.hash, "abc123");
        assert_eq!(meta.size, 1024);
        assert!(!meta.has_outboard);

        // List blobs
        let blobs = db.list_blobs().await.unwrap();
        assert_eq!(blobs.len(), 1);
        assert_eq!(blobs[0], "abc123");

        // Count and total size
        assert_eq!(db.count_blobs().await.unwrap(), 1);
        assert_eq!(db.total_size().await.unwrap(), 1024);

        // Delete
        assert!(db.delete_blob("abc123").await.unwrap());
        assert!(!db.has_blob("abc123").await.unwrap());
    }

    #[tokio::test]
    async fn test_upsert_blob() {
        let db = Database::in_memory().await.unwrap();

        // Insert
        db.insert_blob("abc123", 1024, false).await.unwrap();

        // Update via upsert
        db.insert_blob("abc123", 2048, true).await.unwrap();

        // Verify updated
        let meta = db.get_blob("abc123").await.unwrap().unwrap();
        assert_eq!(meta.size, 2048);
        assert!(meta.has_outboard);

        // Should still be only one blob
        assert_eq!(db.count_blobs().await.unwrap(), 1);
    }
}
