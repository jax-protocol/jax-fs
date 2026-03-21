use sqlx::FromRow;

use crate::database::Database;

/// A cached gateway response lookup result.
#[derive(Debug, Clone, FromRow)]
pub struct GatewayCacheEntry {
    pub content_hash: String,
    pub mime_type: String,
}

/// Parameters for inserting/upserting a cache entry.
pub struct UpsertParams<'a> {
    pub bucket_id: &'a str,
    pub height: i64,
    pub path: &'a str,
    pub query_string: &'a str,
    pub content_hash: &'a str,
    pub content_size: i64,
    pub mime_type: &'a str,
}

impl GatewayCacheEntry {
    /// Look up a cached entry by its key.
    pub async fn lookup(
        bucket_id: &str,
        height: i64,
        path: &str,
        query_string: &str,
        db: &Database,
    ) -> Result<Option<Self>, sqlx::Error> {
        let entry = sqlx::query_as::<_, Self>(
            "SELECT content_hash, mime_type FROM gateway_cache
             WHERE bucket_id = ? AND height = ? AND path = ? AND query_string = ?",
        )
        .bind(bucket_id)
        .bind(height)
        .bind(path)
        .bind(query_string)
        .fetch_optional(&**db)
        .await?;

        // Touch last_accessed on hit
        if entry.is_some() {
            let _ = sqlx::query(
                "UPDATE gateway_cache SET last_accessed = unixepoch()
                 WHERE bucket_id = ? AND height = ? AND path = ? AND query_string = ?",
            )
            .bind(bucket_id)
            .bind(height)
            .bind(path)
            .bind(query_string)
            .execute(&**db)
            .await;
        }

        Ok(entry)
    }

    /// Insert or replace a cache entry.
    pub async fn upsert(params: &UpsertParams<'_>, db: &Database) -> Result<(), sqlx::Error> {
        let UpsertParams {
            bucket_id,
            height,
            path,
            query_string,
            content_hash,
            content_size,
            mime_type,
        } = params;
        sqlx::query(
            "INSERT OR REPLACE INTO gateway_cache
             (bucket_id, height, path, query_string, content_hash, content_size, mime_type,
              created_at, last_accessed)
             VALUES (?, ?, ?, ?, ?, ?, ?, unixepoch(), unixepoch())",
        )
        .bind(bucket_id)
        .bind(height)
        .bind(path)
        .bind(query_string)
        .bind(content_hash)
        .bind(content_size)
        .bind(mime_type)
        .execute(&**db)
        .await?;

        Ok(())
    }

    /// Remove entries for old heights, keeping only the most recent `keep_versions` per bucket.
    pub async fn evict_old_heights(keep_versions: u32, db: &Database) -> Result<u64, sqlx::Error> {
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
        .execute(&**db)
        .await?;

        Ok(result.rows_affected())
    }

    /// Remove LRU entries until total size is under the limit.
    /// Returns the content hashes of removed entries.
    pub async fn evict_lru(max_total_size: i64, db: &Database) -> Result<Vec<String>, sqlx::Error> {
        let total: i64 =
            sqlx::query_scalar("SELECT COALESCE(SUM(content_size), 0) FROM gateway_cache")
                .fetch_one(&**db)
                .await?;

        if total <= max_total_size {
            return Ok(Vec::new());
        }

        let to_free = total - max_total_size;
        let mut freed: i64 = 0;
        let mut removed_hashes = Vec::new();

        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT rowid, content_hash FROM gateway_cache ORDER BY last_accessed ASC",
        )
        .fetch_all(&**db)
        .await?;

        for (rowid, hash) in rows {
            if freed >= to_free {
                break;
            }

            let size: i64 =
                sqlx::query_scalar("SELECT content_size FROM gateway_cache WHERE rowid = ?")
                    .bind(rowid)
                    .fetch_one(&**db)
                    .await?;

            sqlx::query("DELETE FROM gateway_cache WHERE rowid = ?")
                .bind(rowid)
                .execute(&**db)
                .await?;

            freed += size;
            removed_hashes.push(hash);
        }

        Ok(removed_hashes)
    }

    /// Remove entries older than `max_age_secs`.
    /// Returns the content hashes of removed entries.
    pub async fn evict_expired(
        max_age_secs: i64,
        db: &Database,
    ) -> Result<Vec<String>, sqlx::Error> {
        let hashes: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT content_hash FROM gateway_cache
             WHERE created_at < unixepoch() - ?",
        )
        .bind(max_age_secs)
        .fetch_all(&**db)
        .await?;

        sqlx::query("DELETE FROM gateway_cache WHERE created_at < unixepoch() - ?")
            .bind(max_age_secs)
            .execute(&**db)
            .await?;

        Ok(hashes.into_iter().map(|(h,)| h).collect())
    }

    /// Get all content hashes still referenced in the index.
    pub async fn referenced_hashes(db: &Database) -> Result<Vec<String>, sqlx::Error> {
        let hashes: Vec<(String,)> =
            sqlx::query_as("SELECT DISTINCT content_hash FROM gateway_cache")
                .fetch_all(&**db)
                .await?;

        Ok(hashes.into_iter().map(|(h,)| h).collect())
    }

    /// Count total entries (for tests).
    #[cfg(test)]
    pub async fn count(db: &Database) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar("SELECT COUNT(*) FROM gateway_cache")
            .fetch_one(&**db)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;

    async fn test_db() -> Database {
        let id = uuid::Uuid::new_v4();
        let url =
            url::Url::parse(&format!("sqlite:file:test_{id}?mode=memory&cache=shared")).unwrap();
        Database::connect(&url).await.unwrap()
    }

    fn params<'a>(
        bucket_id: &'a str,
        height: i64,
        path: &'a str,
        query_string: &'a str,
        content_hash: &'a str,
        content_size: i64,
        mime_type: &'a str,
    ) -> UpsertParams<'a> {
        UpsertParams {
            bucket_id,
            height,
            path,
            query_string,
            content_hash,
            content_size,
            mime_type,
        }
    }

    #[tokio::test]
    async fn test_insert_and_lookup() {
        let db = test_db().await;

        // Miss
        assert!(GatewayCacheEntry::lookup("b1", 1, "/photo.jpg", "", &db)
            .await
            .unwrap()
            .is_none());

        // Insert and hit
        GatewayCacheEntry::upsert(
            &params("b1", 1, "/photo.jpg", "", "abc123", 1024, "image/jpeg"),
            &db,
        )
        .await
        .unwrap();

        let entry = GatewayCacheEntry::lookup("b1", 1, "/photo.jpg", "", &db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(entry.content_hash, "abc123");
        assert_eq!(entry.mime_type, "image/jpeg");
    }

    #[tokio::test]
    async fn test_query_string_differentiates_entries() {
        let db = test_db().await;

        GatewayCacheEntry::upsert(
            &params(
                "b1",
                1,
                "/photo.jpg",
                "",
                "hash-original",
                5000,
                "image/jpeg",
            ),
            &db,
        )
        .await
        .unwrap();
        GatewayCacheEntry::upsert(
            &params(
                "b1",
                1,
                "/photo.jpg",
                "w=200",
                "hash-thumb",
                500,
                "image/jpeg",
            ),
            &db,
        )
        .await
        .unwrap();

        let original = GatewayCacheEntry::lookup("b1", 1, "/photo.jpg", "", &db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(original.content_hash, "hash-original");

        let thumb = GatewayCacheEntry::lookup("b1", 1, "/photo.jpg", "w=200", &db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(thumb.content_hash, "hash-thumb");
    }

    #[tokio::test]
    async fn test_evict_old_heights() {
        let db = test_db().await;

        for h in 1..=3 {
            let hash = format!("hash-{}", h);
            GatewayCacheEntry::upsert(
                &params("b1", h, "/file.txt", "", &hash, 100, "text/plain"),
                &db,
            )
            .await
            .unwrap();
        }
        assert_eq!(GatewayCacheEntry::count(&db).await.unwrap(), 3);

        let removed = GatewayCacheEntry::evict_old_heights(1, &db).await.unwrap();
        assert_eq!(removed, 2);
        assert_eq!(GatewayCacheEntry::count(&db).await.unwrap(), 1);

        assert!(GatewayCacheEntry::lookup("b1", 3, "/file.txt", "", &db)
            .await
            .unwrap()
            .is_some());
        assert!(GatewayCacheEntry::lookup("b1", 1, "/file.txt", "", &db)
            .await
            .unwrap()
            .is_none());
    }
}
