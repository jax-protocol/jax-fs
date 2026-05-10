use sqlx::Row;
use uuid::Uuid;

use common::bucket_log::BucketAclStatus;

use crate::database::types::BucketAclEvent;
use crate::database::Database;

impl Database {
    /// Append an ACL event to the log.
    pub async fn append_acl_event(
        &self,
        bucket_id: &Uuid,
        event: BucketAclEvent,
        actor: &str,
    ) -> Result<(), sqlx::Error> {
        let id_str = bucket_id.to_string();
        let event_str = event.as_str();
        sqlx::query(
            "INSERT INTO bucket_acl_log (bucket_id, event, actor, created_at) \
             VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)",
        )
        .bind(&id_str)
        .bind(event_str)
        .bind(actor)
        .execute(&**self)
        .await?;

        Ok(())
    }

    /// Get the effective ACL status for a bucket by reading the last event.
    ///
    /// Returns `Some(status)` if there are ACL events or bucket_log entries.
    /// Legacy buckets (bucket_log entries but no ACL rows) return `Some(Active)`.
    /// Returns `None` if the bucket is completely unknown.
    pub async fn get_effective_acl_status(
        &self,
        bucket_id: &Uuid,
    ) -> Result<Option<BucketAclStatus>, sqlx::Error> {
        let id_str = bucket_id.to_string();

        // Check for latest ACL event
        let row = sqlx::query(
            "SELECT event FROM bucket_acl_log \
             WHERE bucket_id = ?1 \
             ORDER BY id DESC LIMIT 1",
        )
        .bind(&id_str)
        .fetch_optional(&**self)
        .await?;

        if let Some(r) = row {
            let event_str: String = r.get("event");
            let event: BucketAclEvent =
                event_str.parse().map_err(|e| sqlx::Error::ColumnDecode {
                    index: "event".to_string(),
                    source: Box::new(e),
                })?;
            return Ok(Some(event.to_status()));
        }

        // No ACL rows — check if bucket_log entries exist (backward compat)
        let exists = sqlx::query("SELECT 1 FROM bucket_log WHERE bucket_id = ?1 LIMIT 1")
            .bind(&id_str)
            .fetch_optional(&**self)
            .await?;

        if exists.is_some() {
            // Legacy bucket: has log entries but no ACL rows → treat as Active
            Ok(Some(BucketAclStatus::Active))
        } else {
            Ok(None)
        }
    }

    /// List all ACL events for a bucket, ordered chronologically.
    pub async fn list_acl_events(
        &self,
        bucket_id: &Uuid,
    ) -> Result<Vec<(BucketAclEvent, String, String)>, sqlx::Error> {
        let id_str = bucket_id.to_string();
        let rows = sqlx::query(
            "SELECT event, actor, created_at FROM bucket_acl_log \
             WHERE bucket_id = ?1 \
             ORDER BY id ASC",
        )
        .bind(&id_str)
        .fetch_all(&**self)
        .await?;

        let mut events = Vec::new();
        for r in rows {
            let event_str: String = r.get("event");
            let actor: String = r.get("actor");
            let created_at: String = r.get("created_at");
            let event: BucketAclEvent =
                event_str.parse().map_err(|e| sqlx::Error::ColumnDecode {
                    index: "event".to_string(),
                    source: Box::new(e),
                })?;
            events.push((event, actor, created_at));
        }

        Ok(events)
    }
}
