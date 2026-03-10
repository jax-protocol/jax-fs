//! Sync event types for FUSE cache invalidation
//!
//! These events are emitted by the sync worker when bucket state changes,
//! allowing FUSE filesystems to invalidate their caches.

use uuid::Uuid;

/// Events emitted by the sync engine for FUSE integration
#[derive(Debug, Clone)]
pub enum SyncEvent {
    /// A bucket's state has been updated (new manifest synced)
    BucketUpdated { bucket_id: Uuid },

    /// A specific mount should invalidate its cache
    MountInvalidated { mount_id: Uuid },
}
