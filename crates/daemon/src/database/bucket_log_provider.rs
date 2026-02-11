use async_trait::async_trait;
use uuid::Uuid;

use common::bucket_log::BucketLogProvider;
use common::linked_data::Link;

use crate::database::{types::DCid, Database};

#[async_trait]
impl BucketLogProvider for Database {
    type Error = sqlx::Error;

    async fn exists(
        &self,
        id: Uuid,
    ) -> Result<bool, common::bucket_log::BucketLogError<Self::Error>> {
        let id_str = id.to_string();
        let result = sqlx::query!(
            r#"
            SELECT COUNT(*) as "count!: i64"
            FROM bucket_log
            WHERE bucket_id = $1
            "#,
            id_str
        )
        .fetch_one(&**self)
        .await
        .map_err(common::bucket_log::BucketLogError::Provider)?;

        Ok(result.count > 0)
    }

    async fn heads(
        &self,
        id: Uuid,
        height: u64,
    ) -> Result<Vec<Link>, common::bucket_log::BucketLogError<Self::Error>> {
        let height_i64 = height as i64;
        let id_str = id.to_string();

        let rows = sqlx::query!(
            r#"
            SELECT current_link as "current_link!: DCid"
            FROM bucket_log
            WHERE bucket_id = $1 AND height = $2
            "#,
            id_str,
            height_i64
        )
        .fetch_all(&**self)
        .await
        .map_err(common::bucket_log::BucketLogError::Provider)?;

        Ok(rows.into_iter().map(|r| r.current_link.into()).collect())
    }

    async fn append(
        &self,
        id: Uuid,
        name: String,
        current: Link,
        previous: Option<Link>,
        height: u64,
        published: bool,
    ) -> Result<(), common::bucket_log::BucketLogError<Self::Error>> {
        let current_dcid: DCid = current.clone().into();
        let previous_dcid: Option<DCid> = previous.clone().map(Into::into);
        let height_i64 = height as i64;

        // Validate: For genesis (previous_link is None), height should be 0
        if previous.is_none() && height != 0 {
            return Err(common::bucket_log::BucketLogError::InvalidAppend(
                current,
                Link::default(),
                height,
            ));
        }

        // For non-genesis, validate that previous link exists at height - 1
        if let Some(prev_link) = previous.clone() {
            if height == 0 {
                return Err(common::bucket_log::BucketLogError::InvalidAppend(
                    current, prev_link, height,
                ));
            }

            let prev_dcid: DCid = prev_link.clone().into();
            let prev_height = (height - 1) as i64;
            let id_str = id.to_string();

            let exists = sqlx::query!(
                r#"
                SELECT COUNT(*) as count
                FROM bucket_log
                WHERE bucket_id = $1 AND current_link = $2 AND height = $3
                "#,
                id_str,
                prev_dcid,
                prev_height
            )
            .fetch_one(&**self)
            .await
            .map_err(common::bucket_log::BucketLogError::Provider)?;

            if exists.count == 0 {
                return Err(common::bucket_log::BucketLogError::InvalidAppend(
                    current, prev_link, height,
                ));
            }
        }

        // Insert the log entry with name
        let id_str = id.to_string();
        sqlx::query!(
            r#"
            INSERT INTO bucket_log (bucket_id, name, current_link, previous_link, height, published, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP)
            "#,
            id_str,
            name,
            current_dcid,
            previous_dcid,
            height_i64,
            published
        )
        .execute(&**self)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(ref db_error) => {
                if db_error.constraint().is_some() {
                    common::bucket_log::BucketLogError::Conflict
                } else {
                    common::bucket_log::BucketLogError::Provider(e)
                }
            }
            _ => common::bucket_log::BucketLogError::Provider(e),
        })?;

        Ok(())
    }

    async fn height(
        &self,
        id: Uuid,
    ) -> Result<u64, common::bucket_log::BucketLogError<Self::Error>> {
        let id_str = id.to_string();
        let result = sqlx::query!(
            r#"
            SELECT MAX(height) as "max_height: i64"
            FROM bucket_log
            WHERE bucket_id = $1
            "#,
            id_str
        )
        .fetch_one(&**self)
        .await
        .map_err(common::bucket_log::BucketLogError::Provider)?;

        match result.max_height {
            Some(h) => Ok(h as u64),
            None => Err(common::bucket_log::BucketLogError::HeadNotFound(0)),
        }
    }

    async fn has(
        &self,
        id: Uuid,
        link: Link,
    ) -> Result<Vec<u64>, common::bucket_log::BucketLogError<Self::Error>> {
        let dcid: DCid = link.into();
        let id_str = id.to_string();

        let rows = sqlx::query!(
            r#"
            SELECT height
            FROM bucket_log
            WHERE bucket_id = $1 AND current_link = $2
            "#,
            id_str,
            dcid
        )
        .fetch_all(&**self)
        .await
        .map_err(common::bucket_log::BucketLogError::Provider)?;

        Ok(rows.into_iter().map(|r| r.height as u64).collect())
    }

    async fn list_buckets(
        &self,
    ) -> Result<Vec<Uuid>, common::bucket_log::BucketLogError<Self::Error>> {
        let rows = sqlx::query!(
            r#"
            SELECT DISTINCT bucket_id
            FROM bucket_log
            ORDER BY bucket_id
            "#
        )
        .fetch_all(&**self)
        .await
        .map_err(common::bucket_log::BucketLogError::Provider)?;

        Ok(rows
            .into_iter()
            .map(|r| Uuid::parse_str(&r.bucket_id).expect("invalid bucket_id UUID in database"))
            .collect())
    }

    async fn latest_published(
        &self,
        id: Uuid,
    ) -> Result<Option<(Link, u64)>, common::bucket_log::BucketLogError<Self::Error>> {
        let id_str = id.to_string();

        let result = sqlx::query!(
            r#"
            SELECT current_link as "current_link!: DCid", height
            FROM bucket_log
            WHERE bucket_id = $1 AND published = TRUE
            ORDER BY height DESC
            LIMIT 1
            "#,
            id_str
        )
        .fetch_optional(&**self)
        .await
        .map_err(common::bucket_log::BucketLogError::Provider)?;

        Ok(result.map(|r| (r.current_link.into(), r.height as u64)))
    }
}
