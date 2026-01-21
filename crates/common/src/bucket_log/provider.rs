use std::fmt::{Debug, Display};

use async_trait::async_trait;
use uuid::Uuid;

use crate::linked_data::Link;

// TODO (amiller68): it might be easier to design this to work
//  with dependency injection over a generic type

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum BucketLogError<T> {
    /// The bucket log is empty
    #[error("unhandled bucket log provider error: {0}")]
    Provider(#[from] T),
    /// The bucket log is empty
    #[error("head not found at height {0}")]
    HeadNotFound(u64),
    /// An append causes a conflict with the current of the
    ///  log i.e. same link at the same height
    #[error("conflict with current log entry")]
    Conflict,
    /// An append does not implement a valid link structure
    ///  st the previous link pointed at by the new log does
    ///  not exist in the log at the expected height --
    ///  current, previous, height
    #[error("invalid append: {0}, {1}, {2}")]
    InvalidAppend(Link, Link, u64),
}

#[async_trait]
pub trait BucketLogProvider: Send + Sync + std::fmt::Debug + Clone + 'static {
    type Error: Display + Debug;

    async fn exists(&self, id: Uuid) -> Result<bool, BucketLogError<Self::Error>>;

    /// Get the possible heads for a bucket
    ///  based on passed height
    ///
    /// # Arguments
    /// * `id` - The UUID of the bucket
    /// * `height` - The height to query the candidate heads for
    ///
    /// # Returns
    /// * `Ok(Vec<Link>)` - The candidate heads for the bucket
    /// * `Err(Self::Error)` - An error occurred while fetching the candidate heads
    async fn heads(&self, id: Uuid, height: u64) -> Result<Vec<Link>, BucketLogError<Self::Error>>;

    // NOTE (amiller68): maybe name is more of a
    //  implementation detail or product concern,
    //  but maybe its not such thing to mandate a
    //  cache for.
    /// Append a version of the bucket to the log
    ///
    /// # Arguments
    /// * `id` - The UUID of the bucket
    /// * `name` - The friendly name for the bucket
    /// * `current` - The current link of the record
    /// * `previous` - The previous link of the record
    /// * `height` - The reported depth of the bucket version within the chain
    /// * `published` - Whether this version is published (mirrors can decrypt)
    ///
    /// Should fail with the following errors to be considered
    ///  correct:
    /// * `Err(BucketLogError::Conflict)` - The append causes a conflict with the current log
    /// * `Err(BucketLogError::InvalidHeight)` - The height is not greater than the previous height
    async fn append(
        &self,
        id: Uuid,
        name: String,
        current: Link,
        // NOTE (amiller68): this should *only*
        //  be null for the genesis of a bucket
        previous: Option<Link>,
        height: u64,
        published: bool,
    ) -> Result<(), BucketLogError<Self::Error>>;

    /// Return the greatest height of the bucket version within the chain
    ///
    /// # Arguments
    /// * `id` - The UUID of the bucket
    ///
    /// # Returns
    /// * `Result<u64, BucketLogError<Self::Error>>` - The height of the bucket version within the chain
    ///
    /// NOTE: while this returns a BucketLogError, it should only ever return a BucketLogError::NotFound
    ///  or ProviderError
    async fn height(&self, id: Uuid) -> Result<u64, BucketLogError<Self::Error>>;

    /// Check if a link exists within a bucket
    ///
    /// # Arguments
    /// * `id` - The UUID of the bucket
    /// * `link` - The link to check for existence as current
    ///
    /// # Returns
    /// * `Result<Vec<u64>, BucketLogError<Self::Error>>`
    ///     The heights at which the link exists within the bucket
    async fn has(&self, id: Uuid, link: Link) -> Result<Vec<u64>, BucketLogError<Self::Error>>;

    /// Get the peers canonical head based on its log entries
    async fn head(
        &self,
        id: Uuid,
        height: Option<u64>,
    ) -> Result<(Link, u64), BucketLogError<Self::Error>> {
        let height = height.unwrap_or(self.height(id).await?);
        let heads = self.heads(id, height).await?;
        Ok((
            heads
                .into_iter()
                .max()
                .ok_or(BucketLogError::HeadNotFound(height))?,
            height,
        ))
    }

    /// List all bucket IDs that have log entries
    ///
    /// # Returns
    /// * `Ok(Vec<Uuid>)` - The list of bucket IDs
    /// * `Err(BucketLogError)` - An error occurred while fetching bucket IDs
    async fn list_buckets(&self) -> Result<Vec<Uuid>, BucketLogError<Self::Error>>;

    /// Get the latest published version of a bucket
    ///
    /// # Arguments
    /// * `id` - The UUID of the bucket
    ///
    /// # Returns
    /// * `Ok(Some((link, height)))` - The latest published version's link and height
    /// * `Ok(None)` - No published version exists
    /// * `Err(BucketLogError)` - An error occurred while fetching
    async fn latest_published(
        &self,
        id: Uuid,
    ) -> Result<Option<(Link, u64)>, BucketLogError<Self::Error>>;
}
