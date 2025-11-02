use async_trait::async_trait;
use uuid::Uuid;

use common::bucket_log_provider::BucketLogProvider;
use common::linked_data::Link;

use crate::database::{types::DCid, Database};

#[async_trait]
impl BucketLogProvider for Database {
    type Error = sqlx::Error;

    async fn heads(
        &self,
        id: Uuid,
        height: u64,
    ) -> Result<Vec<Link>, common::bucket_log_provider::BucketLogError<Self::Error>> {
        let height_i64 = height as i64;

        let rows = sqlx::query!(
            r#"
            SELECT current_link as "current_link!: DCid"
            FROM bucket_log
            WHERE bucket_id = $1 AND height = $2
            "#,
            id,
            height_i64
        )
        .fetch_all(&**self)
        .await
        .map_err(common::bucket_log_provider::BucketLogError::Provider)?;

        Ok(rows.into_iter().map(|r| r.current_link.into()).collect())
    }

    async fn append(
        &self,
        id: Uuid,
        name: String,
        current: Link,
        previous: Option<Link>,
        height: u64,
    ) -> Result<(), common::bucket_log_provider::BucketLogError<Self::Error>> {
        let current_dcid: DCid = current.clone().into();
        let previous_dcid: Option<DCid> = previous.clone().map(Into::into);
        let height_i64 = height as i64;

        // Validate: For genesis (previous_link is None), height should be 0
        if previous.is_none() && height != 0 {
            return Err(common::bucket_log_provider::BucketLogError::InvalidAppend(
                current,
                Link::default(),
                height,
            ));
        }

        // For non-genesis, validate that previous link exists at height - 1
        if let Some(prev_link) = previous.clone() {
            if height == 0 {
                return Err(common::bucket_log_provider::BucketLogError::InvalidAppend(
                    current,
                    prev_link,
                    height,
                ));
            }

            let prev_dcid: DCid = prev_link.clone().into();
            let prev_height = (height - 1) as i64;

            let exists = sqlx::query!(
                r#"
                SELECT COUNT(*) as count
                FROM bucket_log
                WHERE bucket_id = $1 AND current_link = $2 AND height = $3
                "#,
                id,
                prev_dcid,
                prev_height
            )
            .fetch_one(&**self)
            .await
            .map_err(common::bucket_log_provider::BucketLogError::Provider)?;

            if exists.count == 0 {
                return Err(common::bucket_log_provider::BucketLogError::InvalidAppend(
                    current,
                    prev_link,
                    height,
                ));
            }
        }

        // Insert the log entry
        sqlx::query!(
            r#"
            INSERT INTO bucket_log (bucket_id, current_link, previous_link, height, created_at)
            VALUES ($1, $2, $3, $4, CURRENT_TIMESTAMP)
            "#,
            id,
            current_dcid,
            previous_dcid,
            height_i64
        )
        .execute(&**self)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(ref db_error) => {
                if db_error.constraint().is_some() {
                    common::bucket_log_provider::BucketLogError::Conflict
                } else {
                    common::bucket_log_provider::BucketLogError::Provider(e)
                }
            }
            _ => common::bucket_log_provider::BucketLogError::Provider(e),
        })?;

        // Update the bucket name in the buckets table
        sqlx::query!(
            r#"
            UPDATE buckets
            SET name = $1, updated_at = CURRENT_TIMESTAMP
            WHERE id = $2
            "#,
            name,
            id
        )
        .execute(&**self)
        .await
        .map_err(common::bucket_log_provider::BucketLogError::Provider)?;

        Ok(())
    }

    async fn height(
        &self,
        id: Uuid,
    ) -> Result<u64, common::bucket_log_provider::BucketLogError<Self::Error>> {
        let result = sqlx::query!(
            r#"
            SELECT MAX(height) as "max_height: i64"
            FROM bucket_log
            WHERE bucket_id = $1
            "#,
            id
        )
        .fetch_one(&**self)
        .await
        .map_err(common::bucket_log_provider::BucketLogError::Provider)?;

        match result.max_height {
            Some(h) => Ok(h as u64),
            None => Err(common::bucket_log_provider::BucketLogError::HeadNotFound(0)),
        }
    }

    async fn has(
        &self,
        id: Uuid,
        link: Link,
    ) -> Result<Vec<u64>, common::bucket_log_provider::BucketLogError<Self::Error>> {
        let dcid: DCid = link.into();

        let rows = sqlx::query!(
            r#"
            SELECT height
            FROM bucket_log
            WHERE bucket_id = $1 AND current_link = $2
            "#,
            id,
            dcid
        )
        .fetch_all(&**self)
        .await
        .map_err(common::bucket_log_provider::BucketLogError::Provider)?;

        Ok(rows.into_iter().map(|r| r.height as u64).collect())
    }
}
