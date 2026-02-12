use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::database::types::{DBool, DUuid, MountStatus};
use crate::database::Database;

/// FUSE mount configuration stored in database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FuseMount {
    pub mount_id: DUuid,
    pub bucket_id: DUuid,
    pub mount_point: String,
    pub enabled: DBool,
    pub auto_mount: DBool,
    pub read_only: DBool,
    pub cache_size_mb: i64,
    pub cache_ttl_secs: i64,
    pub status: MountStatus,
    pub error_message: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl FuseMount {
    /// Create a new FUSE mount configuration
    pub async fn create(
        bucket_id: Uuid,
        mount_point: &str,
        auto_mount: bool,
        read_only: bool,
        cache_size_mb: Option<i64>,
        cache_ttl_secs: Option<i64>,
        db: &Database,
    ) -> Result<FuseMount, sqlx::Error> {
        let mount_id = DUuid::new();
        let bucket_id = DUuid::from(bucket_id);
        let cache_size = cache_size_mb.unwrap_or(100);
        let cache_ttl = cache_ttl_secs.unwrap_or(60);

        sqlx::query(
            r#"
            INSERT INTO fuse_mounts (
                mount_id, bucket_id, mount_point, auto_mount, read_only,
                cache_size_mb, cache_ttl_secs
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(mount_id)
        .bind(bucket_id)
        .bind(mount_point)
        .bind(auto_mount)
        .bind(read_only)
        .bind(cache_size)
        .bind(cache_ttl)
        .execute(&**db)
        .await?;

        Self::get(*mount_id, db)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    /// Get a FUSE mount by ID
    pub async fn get(mount_id: Uuid, db: &Database) -> Result<Option<FuseMount>, sqlx::Error> {
        let mount_id = DUuid::from(mount_id);
        sqlx::query_as::<_, FuseMount>(
            r#"
            SELECT
                mount_id, bucket_id, mount_point, enabled, auto_mount,
                read_only, cache_size_mb, cache_ttl_secs, status,
                error_message, created_at, updated_at
            FROM fuse_mounts
            WHERE mount_id = ?1
            "#,
        )
        .bind(mount_id)
        .fetch_optional(&**db)
        .await
    }

    /// List all FUSE mounts
    pub async fn list(db: &Database) -> Result<Vec<FuseMount>, sqlx::Error> {
        sqlx::query_as::<_, FuseMount>(
            r#"
            SELECT
                mount_id, bucket_id, mount_point, enabled, auto_mount,
                read_only, cache_size_mb, cache_ttl_secs, status,
                error_message, created_at, updated_at
            FROM fuse_mounts
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&**db)
        .await
    }

    /// Update a FUSE mount configuration
    #[allow(clippy::too_many_arguments)]
    pub async fn update(
        mount_id: Uuid,
        mount_point: Option<&str>,
        enabled: Option<bool>,
        auto_mount: Option<bool>,
        read_only: Option<bool>,
        cache_size_mb: Option<i64>,
        cache_ttl_secs: Option<i64>,
        db: &Database,
    ) -> Result<Option<FuseMount>, sqlx::Error> {
        let existing = match Self::get(mount_id, db).await? {
            Some(m) => m,
            None => return Ok(None),
        };

        let mount_id = DUuid::from(mount_id);
        let mount_point = mount_point.unwrap_or(&existing.mount_point);
        let enabled = enabled.unwrap_or(*existing.enabled);
        let auto_mount = auto_mount.unwrap_or(*existing.auto_mount);
        let read_only = read_only.unwrap_or(*existing.read_only);
        let cache_size = cache_size_mb.unwrap_or(existing.cache_size_mb);
        let cache_ttl = cache_ttl_secs.unwrap_or(existing.cache_ttl_secs);

        sqlx::query(
            r#"
            UPDATE fuse_mounts
            SET mount_point = ?1, enabled = ?2, auto_mount = ?3, read_only = ?4,
                cache_size_mb = ?5, cache_ttl_secs = ?6, updated_at = CURRENT_TIMESTAMP
            WHERE mount_id = ?7
            "#,
        )
        .bind(mount_point)
        .bind(enabled)
        .bind(auto_mount)
        .bind(read_only)
        .bind(cache_size)
        .bind(cache_ttl)
        .bind(mount_id)
        .execute(&**db)
        .await?;

        Self::get(*mount_id, db).await
    }

    /// Delete a FUSE mount
    pub async fn delete(mount_id: Uuid, db: &Database) -> Result<bool, sqlx::Error> {
        let mount_id = DUuid::from(mount_id);
        let result = sqlx::query("DELETE FROM fuse_mounts WHERE mount_id = ?1")
            .bind(mount_id)
            .execute(&**db)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Update the status of a FUSE mount
    pub async fn update_status(
        mount_id: Uuid,
        status: MountStatus,
        error_message: Option<&str>,
        db: &Database,
    ) -> Result<bool, sqlx::Error> {
        let mount_id = DUuid::from(mount_id);
        let result = sqlx::query(
            r#"
            UPDATE fuse_mounts
            SET status = ?1, error_message = ?2, updated_at = CURRENT_TIMESTAMP
            WHERE mount_id = ?3
            "#,
        )
        .bind(status)
        .bind(error_message)
        .bind(mount_id)
        .execute(&**db)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Get all mounts configured for auto-mount
    pub async fn auto_list(db: &Database) -> Result<Vec<FuseMount>, sqlx::Error> {
        sqlx::query_as::<_, FuseMount>(
            r#"
            SELECT
                mount_id, bucket_id, mount_point, enabled, auto_mount,
                read_only, cache_size_mb, cache_ttl_secs, status,
                error_message, created_at, updated_at
            FROM fuse_mounts
            WHERE auto_mount = 1 AND enabled = 1
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&**db)
        .await
    }

    /// Get mounts by bucket ID
    pub async fn by_bucket(bucket_id: Uuid, db: &Database) -> Result<Vec<FuseMount>, sqlx::Error> {
        let bucket_id = DUuid::from(bucket_id);
        sqlx::query_as::<_, FuseMount>(
            r#"
            SELECT
                mount_id, bucket_id, mount_point, enabled, auto_mount,
                read_only, cache_size_mb, cache_ttl_secs, status,
                error_message, created_at, updated_at
            FROM fuse_mounts
            WHERE bucket_id = ?1
            ORDER BY created_at DESC
            "#,
        )
        .bind(bucket_id)
        .fetch_all(&**db)
        .await
    }
}
