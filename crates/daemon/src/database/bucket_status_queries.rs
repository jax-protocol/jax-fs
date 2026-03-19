use sqlx::Row;
use uuid::Uuid;

use crate::database::types::BucketStatus;
use crate::database::Database;

impl Database {
    /// Get the status of a bucket, returning None if no status row exists.
    pub async fn get_bucket_status(
        &self,
        bucket_id: &Uuid,
    ) -> Result<Option<BucketStatus>, sqlx::Error> {
        let id_str = bucket_id.to_string();
        let row = sqlx::query("SELECT status FROM bucket_status WHERE bucket_id = ?1")
            .bind(&id_str)
            .fetch_optional(&**self)
            .await?;

        Ok(row.map(|r| {
            let s: String = r.get("status");
            s.parse().unwrap()
        }))
    }

    /// Set the status for a bucket (upsert).
    pub async fn set_bucket_status(
        &self,
        bucket_id: &Uuid,
        status: BucketStatus,
        shared_by: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let id_str = bucket_id.to_string();
        let status_str = status.as_str();
        sqlx::query(
            "INSERT INTO bucket_status (bucket_id, status, shared_by, updated_at, created_at)
             VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
             ON CONFLICT(bucket_id) DO UPDATE SET
                 status = ?2,
                 updated_at = CURRENT_TIMESTAMP",
        )
        .bind(&id_str)
        .bind(status_str)
        .bind(shared_by)
        .execute(&**self)
        .await?;

        Ok(())
    }

    /// Get the effective status for a bucket.
    /// Returns Active for buckets with no status row (backward compat).
    pub async fn get_effective_bucket_status(
        &self,
        bucket_id: &Uuid,
    ) -> Result<BucketStatus, sqlx::Error> {
        Ok(self
            .get_bucket_status(bucket_id)
            .await?
            .unwrap_or(BucketStatus::Active))
    }
}
