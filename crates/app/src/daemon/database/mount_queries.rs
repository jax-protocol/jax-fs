use time::OffsetDateTime;
use uuid::Uuid;

use crate::daemon::database::Database;

/// Mount status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountStatus {
    Stopped,
    Running,
    Error,
}

impl MountStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            MountStatus::Stopped => "stopped",
            MountStatus::Running => "running",
            MountStatus::Error => "error",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "running" => MountStatus::Running,
            "error" => MountStatus::Error,
            _ => MountStatus::Stopped,
        }
    }
}

/// FUSE mount configuration and status
#[derive(Debug, Clone)]
pub struct MountInfo {
    pub mount_id: Uuid,
    pub bucket_id: Uuid,
    pub mount_point: String,
    pub enabled: bool,
    pub auto_mount: bool,
    pub read_only: bool,
    pub cache_size_mb: u32,
    pub cache_ttl_secs: u32,
    pub pid: Option<i64>,
    pub status: MountStatus,
    pub error_message: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// Parameters for creating a new mount
#[derive(Debug, Clone)]
pub struct CreateMountParams {
    pub bucket_id: Uuid,
    pub mount_point: String,
    pub auto_mount: bool,
    pub read_only: bool,
    pub cache_size_mb: Option<u32>,
    pub cache_ttl_secs: Option<u32>,
}

/// Parameters for updating a mount
#[derive(Debug, Clone, Default)]
pub struct UpdateMountParams {
    pub enabled: Option<bool>,
    pub auto_mount: Option<bool>,
    pub read_only: Option<bool>,
    pub cache_size_mb: Option<u32>,
    pub cache_ttl_secs: Option<u32>,
}

impl Database {
    /// Create a new mount configuration
    pub async fn create_mount(&self, params: CreateMountParams) -> Result<MountInfo, sqlx::Error> {
        let mount_id = Uuid::new_v4();
        let mount_id_str = mount_id.to_string();
        let bucket_id_str = params.bucket_id.to_string();
        let cache_size = params.cache_size_mb.unwrap_or(100) as i64;
        let cache_ttl = params.cache_ttl_secs.unwrap_or(60) as i64;

        sqlx::query!(
            r#"
            INSERT INTO fuse_mounts (
                mount_id, bucket_id, mount_point, enabled, auto_mount,
                read_only, cache_size_mb, cache_ttl_secs, status
            ) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7, 'stopped')
            "#,
            mount_id_str,
            bucket_id_str,
            params.mount_point,
            params.auto_mount,
            params.read_only,
            cache_size,
            cache_ttl
        )
        .execute(&**self)
        .await?;

        // Return the created mount
        self.get_mount(&mount_id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound)
    }

    /// Get a mount by ID
    pub async fn get_mount(&self, mount_id: &Uuid) -> Result<Option<MountInfo>, sqlx::Error> {
        let mount_id_str = mount_id.to_string();

        let row = sqlx::query!(
            r#"
            SELECT
                mount_id as "mount_id!",
                bucket_id as "bucket_id!",
                mount_point as "mount_point!",
                enabled as "enabled!",
                auto_mount as "auto_mount!",
                read_only as "read_only!",
                cache_size_mb as "cache_size_mb!",
                cache_ttl_secs as "cache_ttl_secs!",
                pid,
                status as "status!",
                error_message,
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM fuse_mounts
            WHERE mount_id = ?1
            "#,
            mount_id_str
        )
        .fetch_optional(&**self)
        .await?;

        Ok(row.map(|r| MountInfo {
            mount_id: Uuid::parse_str(&r.mount_id).expect("invalid mount_id UUID"),
            bucket_id: Uuid::parse_str(&r.bucket_id).expect("invalid bucket_id UUID"),
            mount_point: r.mount_point,
            enabled: r.enabled != 0,
            auto_mount: r.auto_mount != 0,
            read_only: r.read_only != 0,
            cache_size_mb: r.cache_size_mb as u32,
            cache_ttl_secs: r.cache_ttl_secs as u32,
            pid: r.pid,
            status: MountStatus::from_str(&r.status),
            error_message: r.error_message,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }))
    }

    /// Get a mount by mount point
    pub async fn get_mount_by_path(&self, mount_point: &str) -> Result<Option<MountInfo>, sqlx::Error> {
        let row = sqlx::query!(
            r#"
            SELECT
                mount_id as "mount_id!",
                bucket_id as "bucket_id!",
                mount_point as "mount_point!",
                enabled as "enabled!",
                auto_mount as "auto_mount!",
                read_only as "read_only!",
                cache_size_mb as "cache_size_mb!",
                cache_ttl_secs as "cache_ttl_secs!",
                pid,
                status as "status!",
                error_message,
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM fuse_mounts
            WHERE mount_point = ?1
            "#,
            mount_point
        )
        .fetch_optional(&**self)
        .await?;

        Ok(row.map(|r| MountInfo {
            mount_id: Uuid::parse_str(&r.mount_id).expect("invalid mount_id UUID"),
            bucket_id: Uuid::parse_str(&r.bucket_id).expect("invalid bucket_id UUID"),
            mount_point: r.mount_point,
            enabled: r.enabled != 0,
            auto_mount: r.auto_mount != 0,
            read_only: r.read_only != 0,
            cache_size_mb: r.cache_size_mb as u32,
            cache_ttl_secs: r.cache_ttl_secs as u32,
            pid: r.pid,
            status: MountStatus::from_str(&r.status),
            error_message: r.error_message,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }))
    }

    /// List all mounts
    pub async fn list_mounts(&self) -> Result<Vec<MountInfo>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT
                mount_id as "mount_id!",
                bucket_id as "bucket_id!",
                mount_point as "mount_point!",
                enabled as "enabled!",
                auto_mount as "auto_mount!",
                read_only as "read_only!",
                cache_size_mb as "cache_size_mb!",
                cache_ttl_secs as "cache_ttl_secs!",
                pid,
                status as "status!",
                error_message,
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM fuse_mounts
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(&**self)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| MountInfo {
                mount_id: Uuid::parse_str(&r.mount_id).expect("invalid mount_id UUID"),
                bucket_id: Uuid::parse_str(&r.bucket_id).expect("invalid bucket_id UUID"),
                mount_point: r.mount_point,
                enabled: r.enabled != 0,
                auto_mount: r.auto_mount != 0,
                read_only: r.read_only != 0,
                cache_size_mb: r.cache_size_mb as u32,
                cache_ttl_secs: r.cache_ttl_secs as u32,
                pid: r.pid,
                status: MountStatus::from_str(&r.status),
                error_message: r.error_message,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect())
    }

    /// List mounts configured for auto-mount
    pub async fn list_auto_mounts(&self) -> Result<Vec<MountInfo>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT
                mount_id as "mount_id!",
                bucket_id as "bucket_id!",
                mount_point as "mount_point!",
                enabled as "enabled!",
                auto_mount as "auto_mount!",
                read_only as "read_only!",
                cache_size_mb as "cache_size_mb!",
                cache_ttl_secs as "cache_ttl_secs!",
                pid,
                status as "status!",
                error_message,
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM fuse_mounts
            WHERE auto_mount = 1 AND enabled = 1
            ORDER BY created_at ASC
            "#
        )
        .fetch_all(&**self)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| MountInfo {
                mount_id: Uuid::parse_str(&r.mount_id).expect("invalid mount_id UUID"),
                bucket_id: Uuid::parse_str(&r.bucket_id).expect("invalid bucket_id UUID"),
                mount_point: r.mount_point,
                enabled: r.enabled != 0,
                auto_mount: r.auto_mount != 0,
                read_only: r.read_only != 0,
                cache_size_mb: r.cache_size_mb as u32,
                cache_ttl_secs: r.cache_ttl_secs as u32,
                pid: r.pid,
                status: MountStatus::from_str(&r.status),
                error_message: r.error_message,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect())
    }

    /// Update mount status and PID
    pub async fn update_mount_status(
        &self,
        mount_id: &Uuid,
        status: MountStatus,
        pid: Option<i64>,
        error_message: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let mount_id_str = mount_id.to_string();
        let status_str = status.as_str();

        sqlx::query!(
            r#"
            UPDATE fuse_mounts
            SET status = ?1, pid = ?2, error_message = ?3, updated_at = CURRENT_TIMESTAMP
            WHERE mount_id = ?4
            "#,
            status_str,
            pid,
            error_message,
            mount_id_str
        )
        .execute(&**self)
        .await?;

        Ok(())
    }

    /// Update mount configuration
    pub async fn update_mount(
        &self,
        mount_id: &Uuid,
        params: UpdateMountParams,
    ) -> Result<Option<MountInfo>, sqlx::Error> {
        let mount_id_str = mount_id.to_string();

        // Build dynamic update query based on what's provided
        // For simplicity, we'll update all fields if any are provided
        if let Some(mount) = self.get_mount(mount_id).await? {
            let enabled = params.enabled.unwrap_or(mount.enabled);
            let auto_mount = params.auto_mount.unwrap_or(mount.auto_mount);
            let read_only = params.read_only.unwrap_or(mount.read_only);
            let cache_size = params.cache_size_mb.unwrap_or(mount.cache_size_mb) as i64;
            let cache_ttl = params.cache_ttl_secs.unwrap_or(mount.cache_ttl_secs) as i64;

            sqlx::query!(
                r#"
                UPDATE fuse_mounts
                SET enabled = ?1, auto_mount = ?2, read_only = ?3,
                    cache_size_mb = ?4, cache_ttl_secs = ?5, updated_at = CURRENT_TIMESTAMP
                WHERE mount_id = ?6
                "#,
                enabled,
                auto_mount,
                read_only,
                cache_size,
                cache_ttl,
                mount_id_str
            )
            .execute(&**self)
            .await?;

            self.get_mount(mount_id).await
        } else {
            Ok(None)
        }
    }

    /// Delete a mount configuration
    pub async fn delete_mount(&self, mount_id: &Uuid) -> Result<bool, sqlx::Error> {
        let mount_id_str = mount_id.to_string();

        let result = sqlx::query!(
            r#"
            DELETE FROM fuse_mounts WHERE mount_id = ?1
            "#,
            mount_id_str
        )
        .execute(&**self)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Get mounts for a specific bucket
    pub async fn get_mounts_for_bucket(&self, bucket_id: &Uuid) -> Result<Vec<MountInfo>, sqlx::Error> {
        let bucket_id_str = bucket_id.to_string();

        let rows = sqlx::query!(
            r#"
            SELECT
                mount_id as "mount_id!",
                bucket_id as "bucket_id!",
                mount_point as "mount_point!",
                enabled as "enabled!",
                auto_mount as "auto_mount!",
                read_only as "read_only!",
                cache_size_mb as "cache_size_mb!",
                cache_ttl_secs as "cache_ttl_secs!",
                pid,
                status as "status!",
                error_message,
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM fuse_mounts
            WHERE bucket_id = ?1
            ORDER BY created_at DESC
            "#,
            bucket_id_str
        )
        .fetch_all(&**self)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| MountInfo {
                mount_id: Uuid::parse_str(&r.mount_id).expect("invalid mount_id UUID"),
                bucket_id: Uuid::parse_str(&r.bucket_id).expect("invalid bucket_id UUID"),
                mount_point: r.mount_point,
                enabled: r.enabled != 0,
                auto_mount: r.auto_mount != 0,
                read_only: r.read_only != 0,
                cache_size_mb: r.cache_size_mb as u32,
                cache_ttl_secs: r.cache_ttl_secs as u32,
                pid: r.pid,
                status: MountStatus::from_str(&r.status),
                error_message: r.error_message,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect())
    }
}
