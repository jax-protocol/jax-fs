use std::path::Path;

use sqlx::FromRow;
use uuid::Uuid;

use crate::database::types::{DBlake3, DMime, DUuid};
use crate::database::Database;

/// A cached gateway response lookup result.
#[derive(Debug, Clone)]
pub struct GatewayCacheEntry {
    /// BLAKE3 hash of the cached content (content-addressed link).
    pub link: blake3::Hash,
    /// MIME type of the cached content.
    pub mime_type: mime::Mime,
}

/// Row type for sqlx deserialization.
#[derive(FromRow)]
struct Row {
    link: DBlake3,
    mime_type: DMime,
}

impl From<Row> for GatewayCacheEntry {
    fn from(row: Row) -> Self {
        Self {
            link: row.link.into(),
            mime_type: row.mime_type.into(),
        }
    }
}

impl GatewayCacheEntry {
    /// Look up a cached entry by its key.
    pub async fn lookup(
        bucket_id: &Uuid,
        height: u64,
        path: &Path,
        query_string: Option<&str>,
        db: &Database,
    ) -> Result<Option<Self>, sqlx::Error> {
        let bid = DUuid::from(*bucket_id);
        let path_str = path.to_string_lossy();
        let qs = query_string.unwrap_or("");
        let row = sqlx::query_as::<_, Row>(
            "SELECT link, mime_type FROM gateway_cache
             WHERE bucket_id = ? AND height = ? AND path = ? AND query_string = ?",
        )
        .bind(bid)
        .bind(height as i64)
        .bind(path_str.as_ref())
        .bind(qs)
        .fetch_optional(&**db)
        .await?;

        // Touch last_accessed on hit
        if row.is_some() {
            let _ = sqlx::query(
                "UPDATE gateway_cache SET last_accessed = unixepoch()
                 WHERE bucket_id = ? AND height = ? AND path = ? AND query_string = ?",
            )
            .bind(bid)
            .bind(height as i64)
            .bind(path_str.as_ref())
            .bind(qs)
            .execute(&**db)
            .await;
        }

        Ok(row.map(Into::into))
    }

    /// Insert or replace a cache entry.
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert(
        bucket_id: &Uuid,
        height: u64,
        path: &Path,
        query_string: Option<&str>,
        link: &blake3::Hash,
        content_size: u64,
        mime_type: &mime::Mime,
        db: &Database,
    ) -> Result<(), sqlx::Error> {
        let bid = DUuid::from(*bucket_id);
        let path_str = path.to_string_lossy();
        let qs = query_string.unwrap_or("");
        let dlink = DBlake3::from(*link);
        let dmime = DMime::from(mime_type.clone());
        sqlx::query(
            "INSERT OR REPLACE INTO gateway_cache
             (bucket_id, height, path, query_string, link, content_size, mime_type,
              created_at, last_accessed)
             VALUES (?, ?, ?, ?, ?, ?, ?, unixepoch(), unixepoch())",
        )
        .bind(bid)
        .bind(height as i64)
        .bind(path_str.as_ref())
        .bind(qs)
        .bind(dlink)
        .bind(content_size as i64)
        .bind(dmime)
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
    /// Returns the links of removed entries.
    pub async fn evict_lru(max_total_size: u64, db: &Database) -> Result<Vec<String>, sqlx::Error> {
        let total: i64 =
            sqlx::query_scalar("SELECT COALESCE(SUM(content_size), 0) FROM gateway_cache")
                .fetch_one(&**db)
                .await?;

        if total <= max_total_size as i64 {
            return Ok(Vec::new());
        }

        let to_free = total - max_total_size as i64;
        let mut freed: i64 = 0;
        let mut removed_links = Vec::new();

        let rows: Vec<(i64, String)> =
            sqlx::query_as("SELECT rowid, link FROM gateway_cache ORDER BY last_accessed ASC")
                .fetch_all(&**db)
                .await?;

        for (rowid, link) in rows {
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
            removed_links.push(link);
        }

        Ok(removed_links)
    }

    /// Remove entries older than `max_age_secs`.
    /// Returns the links of removed entries.
    pub async fn evict_expired(
        max_age_secs: u64,
        db: &Database,
    ) -> Result<Vec<String>, sqlx::Error> {
        let links: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT link FROM gateway_cache
             WHERE created_at < unixepoch() - ?",
        )
        .bind(max_age_secs as i64)
        .fetch_all(&**db)
        .await?;

        sqlx::query("DELETE FROM gateway_cache WHERE created_at < unixepoch() - ?")
            .bind(max_age_secs as i64)
            .execute(&**db)
            .await?;

        Ok(links.into_iter().map(|(h,)| h).collect())
    }

    /// Get all links still referenced in the index.
    pub async fn referenced_links(db: &Database) -> Result<Vec<String>, sqlx::Error> {
        let links: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT link FROM gateway_cache")
            .fetch_all(&**db)
            .await?;

        Ok(links.into_iter().map(|(h,)| h).collect())
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

    #[tokio::test]
    async fn test_insert_and_lookup() {
        let db = Database::memory().await.unwrap();
        let bucket = Uuid::new_v4();
        let link = blake3::hash(b"test content");
        let mime = mime::IMAGE_JPEG;

        // Miss
        assert!(
            GatewayCacheEntry::lookup(&bucket, 1, Path::new("/photo.jpg"), None, &db)
                .await
                .unwrap()
                .is_none()
        );

        // Insert and hit
        GatewayCacheEntry::upsert(
            &bucket,
            1,
            Path::new("/photo.jpg"),
            None,
            &link,
            1024,
            &mime,
            &db,
        )
        .await
        .unwrap();

        let entry = GatewayCacheEntry::lookup(&bucket, 1, Path::new("/photo.jpg"), None, &db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(entry.link, link);
        assert_eq!(entry.mime_type, mime::IMAGE_JPEG);
    }

    #[tokio::test]
    async fn test_query_string_differentiates_entries() {
        let db = Database::memory().await.unwrap();
        let bucket = Uuid::new_v4();
        let link_orig = blake3::hash(b"original");
        let link_thumb = blake3::hash(b"thumbnail");
        let mime = mime::IMAGE_JPEG;

        GatewayCacheEntry::upsert(
            &bucket,
            1,
            Path::new("/photo.jpg"),
            None,
            &link_orig,
            5000,
            &mime,
            &db,
        )
        .await
        .unwrap();
        GatewayCacheEntry::upsert(
            &bucket,
            1,
            Path::new("/photo.jpg"),
            Some("w=200"),
            &link_thumb,
            500,
            &mime,
            &db,
        )
        .await
        .unwrap();

        let original = GatewayCacheEntry::lookup(&bucket, 1, Path::new("/photo.jpg"), None, &db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(original.link, link_orig);

        let thumb =
            GatewayCacheEntry::lookup(&bucket, 1, Path::new("/photo.jpg"), Some("w=200"), &db)
                .await
                .unwrap()
                .unwrap();
        assert_eq!(thumb.link, link_thumb);
    }

    #[tokio::test]
    async fn test_evict_old_heights() {
        let db = Database::memory().await.unwrap();
        let bucket = Uuid::new_v4();

        for h in 1u64..=3 {
            let link = blake3::hash(format!("v{}", h).as_bytes());
            GatewayCacheEntry::upsert(
                &bucket,
                h,
                Path::new("/file.txt"),
                None,
                &link,
                100,
                &mime::TEXT_PLAIN,
                &db,
            )
            .await
            .unwrap();
        }
        assert_eq!(GatewayCacheEntry::count(&db).await.unwrap(), 3);

        let removed = GatewayCacheEntry::evict_old_heights(1, &db).await.unwrap();
        assert_eq!(removed, 2);
        assert_eq!(GatewayCacheEntry::count(&db).await.unwrap(), 1);

        assert!(
            GatewayCacheEntry::lookup(&bucket, 3, Path::new("/file.txt"), None, &db)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            GatewayCacheEntry::lookup(&bucket, 1, Path::new("/file.txt"), None, &db)
                .await
                .unwrap()
                .is_none()
        );
    }
}
