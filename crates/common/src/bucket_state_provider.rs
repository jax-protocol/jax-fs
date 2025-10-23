use async_trait::async_trait;
use uuid::Uuid;

use crate::linked_data::Link;

// TODO (amiller68): no more anyhow::Error!
// TODO (amiller68): kinda debatable whether or not name
//  should be implemented here, since that is implemented
//  as part of a manifest

// use super::messages::SyncStatus;

// TODO (amiller68): this sync status was fuly vibe coded, so
//  i should really validate how *useful* these are
// NOTE (amiller68): without a consistency protocol, these statuses
//  are very loosely defined / not super representative of the actual
//  bucket state e.g. we lack a true definition of 'Synced'
// NOTE (amiller68): part of me thinks this is better left
//  as an implementation level concern, but hey it might
//  be useful to the protocol at some point (like peers being
//  able to say "hold on im syncing this"... idk)
/// Sync status for a bucket
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BucketSyncStatus {
    /// Bucket is fully up to date with a majority of our peers,
    ///  we agree on the current link
    Synced,
    /// Bucket is behind our peers
    OutOfSync,
    /// We are attempting to catch up to our peers,
    ///  or otherwise checking if we are up to date
    Syncing,
    /// Something's wrong, and our sync is broken!
    ///  report a bug!
    Failed,
}

/// Trait for providing bucket tracking state to a peer.
///  Note, this *does not* manage the content of a blob store
///  or handle any syncing! This is a glorified list manager!
/// Think of this mainly as a trait that lets as treat any data store
///  as a backer for a peer's view of its own buckets such as
///  - sqlite
///  - a simple hash map
///  - ReDb
#[async_trait]
pub trait BucketStateProvider: Send + Sync + std::fmt::Debug {
    /// Create a new bucket in the peer's data store
    ///  Used both when creating a new bucket or
    ///  syncing a bucket broadcasted from another peer
    ///
    /// # Args
    /// - `bucket_id`: The unique identifier for the bucket.
    /// - `name`: The human readable name of the bucket.
    /// - `link`: The link associated with the bucket.
    ///
    /// # Returns
    /// - `Ok(())` if the bucket was successfully created.
    /// - `Err(anyhow::Error)` if there was an error creating the bucket.
    async fn create_bucket(
        &self,
        bucket_id: Uuid,
        // NOTE (amiller68): for now, just pass the name,
        //  but we might do away with this, or just not use this
        //  within an implementation
        name: String,
        link: Link,
    ) -> Result<(), anyhow::Error>;

    /// List the available buckets on the state provider
    async fn list_buckets(&self) -> Result<Vec<Uuid>, anyhow::Error>;

    /// Get the current state of a bucket
    async fn get_bucket(&self, bucket_id: Uuid) -> Result<(Link, BucketSyncStatus), anyhow::Error>;

    /// Async update the bucket link + status in the data store
    ///
    /// # Args
    /// - `bucket_id`: The ID of the bucket to update.
    /// - `link`: The new link for the bucket, if any.
    /// - `status`: The new status for the bucket, if any.
    async fn update_bucket(
        &self,
        bucket_id: Uuid,
        link: Option<Link>,
        status: Option<BucketSyncStatus>,
    ) -> Result<(), anyhow::Error>;

    // TODO (amiller68): removing a bucket, but i think that requires some
    //  amount of protocol work as well, so skipping for now
}
