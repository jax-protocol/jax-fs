//! Mount manager for FUSE filesystems
//!
//! Manages the lifecycle of FUSE mounts, keeping them alive and synced.
//! Subscribes to sync events to invalidate caches when bucket state changes.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use fuser::BackgroundSession;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::database::models::FuseMount;
use crate::database::types::MountStatus;
use crate::database::Database;
use crate::fuse::cache::FileCacheConfig;
use crate::fuse::jax_fs::JaxFs;
use crate::fuse::sync_events::SyncEvent;
use crate::fuse::FileCache;
use common::mount::Mount;
use common::peer::Peer;

/// Configuration for mount manager
#[derive(Debug, Clone)]
pub struct MountManagerConfig {
    /// Channel capacity for sync events
    pub sync_event_capacity: usize,
}

impl Default for MountManagerConfig {
    fn default() -> Self {
        Self {
            sync_event_capacity: 256,
        }
    }
}

/// A live mount with its associated state
pub struct LiveMount {
    /// The bucket mount (kept alive for quick access)
    pub mount: Arc<RwLock<Mount>>,
    /// FUSE session handle (if running)
    pub session: Option<BackgroundSession>,
    /// File cache for this mount
    pub cache: FileCache,
    /// Configuration from database
    pub config: FuseMount,
}

impl std::fmt::Debug for LiveMount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveMount")
            .field("config", &self.config)
            .field("has_session", &self.session.is_some())
            .finish()
    }
}

/// Manager for FUSE mounts
///
/// Keeps mounts alive and synced, handling lifecycle and cache invalidation.
pub struct MountManager {
    /// Live mounts: mount_id → LiveMount
    mounts: RwLock<HashMap<Uuid, LiveMount>>,
    /// Database for persistence
    db: Database,
    /// Peer for sync integration
    peer: Peer<Database>,
    /// Sync event broadcaster
    sync_tx: broadcast::Sender<SyncEvent>,
}

impl MountManager {
    /// Create a new mount manager
    pub fn new(db: Database, peer: Peer<Database>, config: MountManagerConfig) -> Self {
        let (sync_tx, _) = broadcast::channel(config.sync_event_capacity);

        Self {
            mounts: RwLock::new(HashMap::new()),
            db,
            peer,
            sync_tx,
        }
    }

    /// Subscribe to sync events
    pub fn subscribe_sync_events(&self) -> broadcast::Receiver<SyncEvent> {
        self.sync_tx.subscribe()
    }

    /// Get a reference to the sync event sender
    pub fn sync_sender(&self) -> broadcast::Sender<SyncEvent> {
        self.sync_tx.clone()
    }

    /// Called when a bucket sync completes - refresh affected mounts
    pub async fn on_bucket_synced(&self, bucket_id: Uuid) -> Result<(), MountError> {
        let mounts = self.mounts.read().await;

        for (mount_id, live_mount) in mounts.iter() {
            if *live_mount.config.bucket_id == bucket_id {
                // Reload mount from updated log head
                let new_mount = self
                    .peer
                    .mount(bucket_id)
                    .await
                    .map_err(|e| MountError::MountLoad(e.into()))?;

                *live_mount.mount.write().await = new_mount;

                // Invalidate cache
                live_mount.cache.invalidate_all();

                // Notify subscribers
                let _ = self.sync_tx.send(SyncEvent::MountInvalidated {
                    mount_id: *mount_id,
                });

                tracing::debug!(
                    "Mount {} cache invalidated after bucket {} sync",
                    mount_id,
                    bucket_id
                );
            }
        }

        Ok(())
    }

    /// Create a new mount configuration
    pub async fn create_mount(
        &self,
        bucket_id: Uuid,
        mount_point: &str,
        auto_mount: bool,
        read_only: bool,
        cache_size_mb: Option<u32>,
        cache_ttl_secs: Option<u32>,
    ) -> Result<FuseMount, MountError> {
        // Validate bucket exists
        let bucket_info = self
            .db
            .get_bucket_info(&bucket_id)
            .await
            .map_err(MountError::Database)?;

        if bucket_info.is_none() {
            return Err(MountError::BucketNotFound(bucket_id));
        }

        // Validate or create mount point
        let mount_path = PathBuf::from(mount_point);
        if !mount_path.exists() {
            std::fs::create_dir_all(&mount_path).map_err(|e| {
                MountError::MountPointNotFound(format!("{} (failed to create: {})", mount_point, e))
            })?;
        }

        if !mount_path.is_dir() {
            return Err(MountError::MountPointNotDirectory(mount_point.to_string()));
        }

        // Create in database
        let mount = FuseMount::create(
            bucket_id,
            mount_point,
            auto_mount,
            read_only,
            cache_size_mb.map(|v| v as i64),
            cache_ttl_secs.map(|v| v as i64),
            &self.db,
        )
        .await
        .map_err(MountError::Database)?;

        tracing::info!(
            "Created mount {} for bucket {} at {}",
            mount.mount_id,
            mount.bucket_id,
            mount.mount_point
        );

        Ok(mount)
    }

    /// Get a mount by ID
    pub async fn get(&self, mount_id: &Uuid) -> Result<Option<FuseMount>, MountError> {
        FuseMount::get(*mount_id, &self.db)
            .await
            .map_err(MountError::Database)
    }

    /// List all mounts
    pub async fn list(&self) -> Result<Vec<FuseMount>, MountError> {
        FuseMount::list(&self.db)
            .await
            .map_err(MountError::Database)
    }

    /// Update a mount configuration
    #[allow(clippy::too_many_arguments)]
    pub async fn update(
        &self,
        mount_id: &Uuid,
        mount_point: Option<&str>,
        enabled: Option<bool>,
        auto_mount: Option<bool>,
        read_only: Option<bool>,
        cache_size_mb: Option<u32>,
        cache_ttl_secs: Option<u32>,
    ) -> Result<Option<FuseMount>, MountError> {
        FuseMount::update(
            *mount_id,
            mount_point,
            enabled,
            auto_mount,
            read_only,
            cache_size_mb.map(|v| v as i64),
            cache_ttl_secs.map(|v| v as i64),
            &self.db,
        )
        .await
        .map_err(MountError::Database)
    }

    /// Delete a mount configuration
    pub async fn delete(&self, mount_id: &Uuid) -> Result<bool, MountError> {
        // First stop if running
        let _ = self.stop(mount_id).await;

        // Remove from live mounts
        self.mounts.write().await.remove(mount_id);

        // Delete from database
        FuseMount::delete(*mount_id, &self.db)
            .await
            .map_err(MountError::Database)
    }

    /// Start a mount (spawn FUSE process)
    pub async fn start(&self, mount_id: &Uuid) -> Result<(), MountError> {
        // Get mount config
        let mount_config = FuseMount::get(*mount_id, &self.db)
            .await
            .map_err(MountError::Database)?
            .ok_or(MountError::MountNotFound(*mount_id))?;

        // Check if already running
        {
            let mounts = self.mounts.read().await;
            if let Some(live) = mounts.get(mount_id) {
                if live.session.is_some() {
                    return Err(MountError::AlreadyRunning(*mount_id));
                }
            }
        }

        // Update status to starting
        FuseMount::update_status(*mount_id, MountStatus::Starting, None, &self.db)
            .await
            .map_err(MountError::Database)?;

        // Get bucket name for volume label
        let bucket_name = self
            .db
            .get_bucket_info(&mount_config.bucket_id)
            .await
            .ok()
            .flatten()
            .map(|info| info.name)
            .unwrap_or_else(|| "jax".to_string());

        // Load the bucket mount
        let bucket_mount = self
            .peer
            .mount(*mount_config.bucket_id)
            .await
            .map_err(|e| MountError::MountLoad(e.into()))?;

        // Create cache
        let cache = FileCache::new(crate::fuse::cache::FileCacheConfig {
            max_size_mb: mount_config.cache_size_mb as u32,
            ttl_secs: mount_config.cache_ttl_secs as u32,
        });

        // Create the FUSE filesystem with direct Mount reference
        let mount_arc = Arc::new(RwLock::new(bucket_mount));
        let sync_rx = self.subscribe_sync_events();

        let fs = JaxFs::new(
            tokio::runtime::Handle::current(),
            mount_arc.clone(),
            *mount_id,
            *mount_config.bucket_id,
            FileCacheConfig {
                max_size_mb: mount_config.cache_size_mb as u32,
                ttl_secs: mount_config.cache_ttl_secs as u32,
            },
            *mount_config.read_only,
            Some(sync_rx),
        );

        // Mount options
        #[cfg(target_os = "linux")]
        let options = vec![
            fuser::MountOption::FSName("jax".to_string()),
            fuser::MountOption::AutoUnmount,
            fuser::MountOption::AllowOther,
        ];

        #[cfg(target_os = "macos")]
        let options = vec![
            fuser::MountOption::FSName("jax".to_string()),
            fuser::MountOption::AutoUnmount,
            fuser::MountOption::CUSTOM(format!("volname={}", bucket_name)),
            fuser::MountOption::CUSTOM("local".to_string()),
            fuser::MountOption::CUSTOM("noappledouble".to_string()),
        ];

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        let options = vec![
            fuser::MountOption::FSName("jax".to_string()),
            fuser::MountOption::AutoUnmount,
        ];

        // Spawn the FUSE session in background
        let mount_path = std::path::Path::new(&mount_config.mount_point);
        tracing::info!("Mounting FUSE filesystem at {:?}", mount_path);

        let session = fuser::spawn_mount2(fs, mount_path, &options).map_err(|e| {
            MountError::SpawnFailed(format!(
                "Failed to mount at {}: {}",
                mount_config.mount_point, e
            ))
        })?;

        // Create live mount
        let live_mount = LiveMount {
            mount: mount_arc,
            session: Some(session),
            cache,
            config: mount_config.clone(),
        };

        // Store in live mounts
        self.mounts.write().await.insert(*mount_id, live_mount);

        // Update status to running
        FuseMount::update_status(*mount_id, MountStatus::Running, None, &self.db)
            .await
            .map_err(MountError::Database)?;

        tracing::info!("Started mount {} at {}", mount_id, mount_config.mount_point);

        Ok(())
    }

    /// Stop a mount
    pub async fn stop(&self, mount_id: &Uuid) -> Result<(), MountError> {
        // Update status to stopping
        let _ = FuseMount::update_status(*mount_id, MountStatus::Stopping, None, &self.db).await;

        let mount_point = {
            let mut mounts = self.mounts.write().await;
            if let Some(live) = mounts.get_mut(mount_id) {
                // Drop the session to unmount
                live.session.take();

                live.config.mount_point.clone()
            } else {
                // Try to get from database
                match FuseMount::get(*mount_id, &self.db).await {
                    Ok(Some(config)) => config.mount_point,
                    _ => return Ok(()),
                }
            }
        };

        // Platform-specific unmount
        self.unmount_path(&mount_point).await?;

        // Update status to stopped
        FuseMount::update_status(*mount_id, MountStatus::Stopped, None, &self.db)
            .await
            .map_err(MountError::Database)?;

        tracing::info!("Stopped mount {} at {}", mount_id, mount_point);

        Ok(())
    }

    /// Stop all running mounts
    pub async fn stop_all(&self) -> Result<(), MountError> {
        let mount_ids: Vec<Uuid> = {
            let mounts = self.mounts.read().await;
            mounts.keys().copied().collect()
        };

        for mount_id in mount_ids {
            if let Err(e) = self.stop(&mount_id).await {
                tracing::error!("Failed to stop mount {}: {}", mount_id, e);
            }
        }

        Ok(())
    }

    /// Start all mounts configured for auto-mount
    pub async fn start_auto(&self) -> Result<(), MountError> {
        let auto_mounts = FuseMount::auto_list(&self.db)
            .await
            .map_err(MountError::Database)?;

        tracing::info!("Starting {} auto-mount(s)", auto_mounts.len());

        for mount in auto_mounts {
            if let Err(e) = self.start(&mount.mount_id).await {
                tracing::error!(
                    "Failed to auto-mount {} at {}: {}",
                    mount.mount_id,
                    mount.mount_point,
                    e
                );

                // Update status to error
                let _ = FuseMount::update_status(
                    *mount.mount_id,
                    MountStatus::Error,
                    Some(&e.to_string()),
                    &self.db,
                )
                .await;
            }
        }

        Ok(())
    }

    /// Get a live mount by ID
    pub async fn get_live_mount(&self, mount_id: &Uuid) -> Option<Arc<RwLock<Mount>>> {
        let mounts = self.mounts.read().await;
        mounts.get(mount_id).map(|m| m.mount.clone())
    }

    /// Get the cache for a live mount
    pub async fn get_mount_cache(&self, mount_id: &Uuid) -> Option<FileCache> {
        let mounts = self.mounts.read().await;
        mounts.get(mount_id).map(|m| m.cache.clone())
    }

    /// Platform-specific unmount
    async fn unmount_path(&self, mount_point: &str) -> Result<(), MountError> {
        use std::process::Command;

        #[cfg(target_os = "macos")]
        {
            let status = Command::new("umount")
                .arg(mount_point)
                .status()
                .map_err(|e| MountError::UnmountFailed(e.to_string()))?;

            if !status.success() {
                // Try diskutil as fallback
                let _ = Command::new("diskutil")
                    .args(["unmount", "force", mount_point])
                    .status();
            }
        }

        #[cfg(target_os = "linux")]
        {
            let status = Command::new("fusermount")
                .args(["-u", mount_point])
                .status()
                .map_err(|e| MountError::UnmountFailed(e.to_string()))?;

            if !status.success() {
                // Try lazy unmount as fallback
                let _ = Command::new("fusermount")
                    .args(["-uz", mount_point])
                    .status();
            }
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            tracing::warn!("Unmount not implemented for this platform");
        }

        Ok(())
    }
}

impl std::fmt::Debug for MountManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MountManager")
            .field("peer_id", &self.peer.id())
            .finish()
    }
}

/// Errors that can occur during mount operations
#[derive(Debug, thiserror::Error)]
pub enum MountError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("bucket not found: {0}")]
    BucketNotFound(Uuid),

    #[error("mount not found: {0}")]
    MountNotFound(Uuid),

    #[error("mount point not found: {0}")]
    MountPointNotFound(String),

    #[error("mount point is not a directory: {0}")]
    MountPointNotDirectory(String),

    #[error("mount already running: {0}")]
    AlreadyRunning(Uuid),

    #[error("failed to load bucket mount: {0}")]
    MountLoad(#[source] anyhow::Error),

    #[error("failed to spawn FUSE process: {0}")]
    SpawnFailed(String),

    #[error("failed to unmount: {0}")]
    UnmountFailed(String),
}
