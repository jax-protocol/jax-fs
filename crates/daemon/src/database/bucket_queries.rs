use time::OffsetDateTime;
use uuid::Uuid;

use common::bucket_log::BucketAclStatus;
use common::prelude::Link;

use crate::database::types::{BucketAclEvent, DCid};
use crate::database::Database;

/// Simple bucket info for UI display (from bucket_log)
#[derive(Debug, Clone)]
pub struct BucketInfo {
    pub id: Uuid,
    pub name: String,
    pub link: Link,
    pub created_at: OffsetDateTime,
}

/// Bucket log entry for history display
#[derive(Debug, Clone)]
pub struct BucketLogEntry {
    pub bucket_id: Uuid,
    pub name: String,
    pub current_link: Link,
    pub previous_link: Option<Link>,
    pub height: u64,
    pub published: bool,
    pub created_at: OffsetDateTime,
}

impl Database {
    /// Get bucket info by ID from the latest bucket_log entry
    pub async fn get_bucket_info(&self, id: &Uuid) -> Result<Option<BucketInfo>, sqlx::Error> {
        let id_str = id.to_string();
        let row = sqlx::query!(
            r#"
            SELECT
                bucket_id as "bucket_id!",
                name as "name!",
                current_link as "current_link!: DCid",
                created_at as "created_at!"
            FROM bucket_log
            WHERE bucket_id = $1
            ORDER BY height DESC
            LIMIT 1
            "#,
            id_str
        )
        .fetch_optional(&**self)
        .await?;

        Ok(row.map(|r| BucketInfo {
            id: Uuid::parse_str(&r.bucket_id).expect("invalid bucket_id UUID in database"),
            name: r.name,
            link: r.current_link.into(),
            created_at: r.created_at,
        }))
    }

    /// List all buckets from the latest bucket_log entries, with effective ACL status.
    pub async fn list_buckets(
        &self,
        prefix: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<(BucketInfo, BucketAclStatus)>, sqlx::Error> {
        let limit_val = limit.unwrap_or(100).min(1000) as i64;
        let pattern = prefix
            .map(|p| format!("{}%", p))
            .unwrap_or_else(|| "%".to_string());

        let rows: Vec<(String, String, DCid, OffsetDateTime, Option<String>)> = sqlx::query_as(
            "SELECT \
                 bl.bucket_id, \
                 bl.name, \
                 bl.current_link, \
                 MIN(bl.created_at) as created_at, \
                 acl.event \
             FROM bucket_log bl \
             INNER JOIN ( \
                 SELECT bucket_id, MAX(height) as max_height \
                 FROM bucket_log \
                 GROUP BY bucket_id \
             ) latest ON bl.bucket_id = latest.bucket_id AND bl.height = latest.max_height \
             LEFT JOIN ( \
                 SELECT bucket_id, event \
                 FROM bucket_acl_log \
                 WHERE id IN (SELECT MAX(id) FROM bucket_acl_log GROUP BY bucket_id) \
             ) acl ON bl.bucket_id = acl.bucket_id \
             WHERE bl.name LIKE ?1 \
             GROUP BY bl.bucket_id, bl.name, bl.current_link, acl.event \
             ORDER BY created_at DESC \
             LIMIT ?2",
        )
        .bind(&pattern)
        .bind(limit_val)
        .fetch_all(&**self)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(bucket_id, name, current_link, created_at, event_str)| {
                let status = match event_str {
                    Some(e) => e
                        .parse::<BucketAclEvent>()
                        .expect("invalid ACL event in database")
                        .to_status(),
                    None => BucketAclStatus::Active,
                };
                (
                    BucketInfo {
                        id: Uuid::parse_str(&bucket_id)
                            .expect("invalid bucket_id UUID in database"),
                        name,
                        link: current_link.into(),
                        created_at,
                    },
                    status,
                )
            })
            .collect())
    }

    /// Get paginated bucket log entries for a specific bucket
    pub async fn get_bucket_logs(
        &self,
        bucket_id: &Uuid,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<BucketLogEntry>, sqlx::Error> {
        let bucket_id_str = bucket_id.to_string();
        let limit = page_size.min(100) as i64;
        let offset = (page * page_size) as i64;

        let rows = sqlx::query!(
            r#"
            SELECT
                bucket_id as "bucket_id!",
                name as "name!",
                current_link as "current_link!: DCid",
                previous_link as "previous_link: DCid",
                height as "height!",
                published as "published!: bool",
                created_at as "created_at!"
            FROM bucket_log
            WHERE bucket_id = ?1
            ORDER BY height DESC
            LIMIT ?2 OFFSET ?3
            "#,
            bucket_id_str,
            limit,
            offset
        )
        .fetch_all(&**self)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| BucketLogEntry {
                bucket_id: Uuid::parse_str(&r.bucket_id)
                    .expect("invalid bucket_id UUID in database"),
                name: r.name,
                current_link: r.current_link.into(),
                previous_link: r.previous_link.map(Into::into),
                height: r.height as u64,
                published: r.published,
                created_at: r.created_at,
            })
            .collect())
    }

    /// Get all bucket log entries for a specific bucket (unpaginated, for tree view)
    pub async fn get_all_bucket_logs(
        &self,
        bucket_id: &Uuid,
    ) -> Result<Vec<BucketLogEntry>, sqlx::Error> {
        let bucket_id_str = bucket_id.to_string();

        let rows = sqlx::query!(
            r#"
            SELECT
                bucket_id as "bucket_id!",
                name as "name!",
                current_link as "current_link!: DCid",
                previous_link as "previous_link: DCid",
                height as "height!",
                published as "published!: bool",
                created_at as "created_at!"
            FROM bucket_log
            WHERE bucket_id = ?1
            ORDER BY height ASC
            "#,
            bucket_id_str
        )
        .fetch_all(&**self)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| BucketLogEntry {
                bucket_id: Uuid::parse_str(&r.bucket_id)
                    .expect("invalid bucket_id UUID in database"),
                name: r.name,
                current_link: r.current_link.into(),
                previous_link: r.previous_link.map(Into::into),
                height: r.height as u64,
                published: r.published,
                created_at: r.created_at,
            })
            .collect())
    }

    /// Get total count of log entries for a bucket
    pub async fn get_bucket_log_count(&self, bucket_id: &Uuid) -> Result<i64, sqlx::Error> {
        let bucket_id_str = bucket_id.to_string();

        let result = sqlx::query!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM bucket_log
            WHERE bucket_id = ?1
            "#,
            bucket_id_str
        )
        .fetch_one(&**self)
        .await?;

        Ok(result.count)
    }
}
