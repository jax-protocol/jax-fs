//! Mount manager for FUSE filesystems
//!
//! Manages the lifecycle of FUSE mounts, keeping them alive and synced.
//! Subscribes to sync events to invalidate caches when bucket state changes.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{broadcast, mpsc, oneshot, RwLock};
use uuid::Uuid;

use crate::database::models::FuseMount;
use crate::database::types::MountStatus;
use crate::database::Database;
use crate::fuse::cache::FileCacheConfig;
use crate::fuse::jax_fs::JaxFs;
use crate::fuse::sync_events::{SaveRequest, SyncEvent};
use crate::fuse::FileCache;
use common::mount::{ConflictFile, Mount};
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

/// Handle to a FUSE session running in a background thread
///
/// This wrapper makes the session Send+Sync by running the actual BackgroundSession
/// in a dedicated thread and communicating via channel.
pub struct SessionHandle {
    /// Sending on this channel signals the session thread to drop the session and unmount
    unmount_tx: Option<oneshot::Sender<()>>,
}

impl SessionHandle {
    /// Create a new session handle that owns the FUSE session in a background thread
    fn spawn(session: fuser::BackgroundSession) -> Self {
        let (unmount_tx, unmount_rx) = oneshot::channel();

        // Spawn a thread that owns the session
        // When unmount_rx receives a message (or is dropped), the session is dropped
        std::thread::spawn(move || {
            // Keep the session alive until we receive the unmount signal
            let _session = session;
            // Block until unmount is requested or the sender is dropped
            let _ = unmount_rx.blocking_recv();
            // Session is dropped here, which unmounts the filesystem
        });

        Self {
            unmount_tx: Some(unmount_tx),
        }
    }

    /// Signal the session to unmount
    fn unmount(&mut self) {
        if let Some(tx) = self.unmount_tx.take() {
            let _ = tx.send(());
        }
    }
}

/// A live mount with its associated state
pub struct LiveMount {
    /// The bucket mount (kept alive for quick access)
    pub mount: Arc<RwLock<Mount>>,
    /// FUSE session handle (if running)
    pub session: Option<SessionHandle>,
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
    ///
    /// If the live mount has unsaved local changes (ops_log non-empty), merges them
    /// with incoming changes using conflict resolution. Otherwise, simply reloads
    /// from the new head.
    pub async fn on_bucket_synced(&self, bucket_id: Uuid) -> Result<(), MountError> {
        let mounts = self.mounts.read().await;

        for (mount_id, live_mount) in mounts.iter() {
            if *live_mount.config.bucket_id == bucket_id {
                // Check if mount has unsaved local changes
                let has_local_changes = {
                    let mount_guard = live_mount.mount.read().await;
                    let inner = mount_guard.inner().await;
                    !inner.ops_log().is_empty()
                };

                if has_local_changes {
                    tracing::info!(
                        "Mount {} has local changes, merging with incoming sync",
                        mount_id
                    );

                    // Load the incoming mount from the new head
                    let incoming = self
                        .peer
                        .mount(bucket_id)
                        .await
                        .map_err(|e| MountError::MountLoad(e.into()))?;

                    // Use ConflictFile resolver to create conflict copies for concurrent edits
                    let resolver = ConflictFile::new();

                    // Merge incoming changes into local mount
                    let mut mount_guard = live_mount.mount.write().await;
                    match mount_guard
                        .merge_from(&incoming, &resolver, self.peer.blobs())
                        .await
                    {
                        Ok((result, link)) => {
                            // Log any conflicts that were resolved
                            for resolved in &result.conflicts_resolved {
                                tracing::info!(
                                    "Resolved conflict for {:?}: {:?}",
                                    resolved.conflict.path,
                                    resolved.resolution
                                );
                            }

                            tracing::info!(
                                "Merged {} operations, {} conflicts resolved, new link: {}",
                                result.operations_added,
                                result.conflicts_resolved.len(),
                                link.hash()
                            );

                            // Save the merged result
                            if let Err(e) = self.peer.save_mount(&mount_guard, false).await {
                                tracing::error!("Failed to save merged mount {}: {}", mount_id, e);
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                "Failed to merge mount {} with incoming changes: {}",
                                mount_id,
                                e
                            );
                            // Fall back to simple reload
                            let new_mount = self
                                .peer
                                .mount(bucket_id)
                                .await
                                .map_err(|e| MountError::MountLoad(e.into()))?;
                            *mount_guard = new_mount;
                        }
                    }
                } else {
                    // No local changes - simple reload from updated head
                    let new_mount = self
                        .peer
                        .mount(bucket_id)
                        .await
                        .map_err(|e| MountError::MountLoad(e.into()))?;

                    *live_mount.mount.write().await = new_mount;
                }

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

        // Create save channel for persistence requests from FUSE
        let (save_tx, save_rx) = mpsc::channel::<SaveRequest>(32);

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
            Some(save_tx),
        );

        // Spawn save handler task
        self.spawn_save_handler(save_rx, mount_arc.clone());

        // Mount options
        #[cfg(target_os = "linux")]
        let options = vec![
            fuser::MountOption::FSName("jax".to_string()),
            fuser::MountOption::AutoUnmount,
            fuser::MountOption::AllowOther,
        ];

        #[cfg(target_os = "macos")]
        let options = {
            // Get bucket name for volume label (macOS only)
            let bucket_name = self
                .db
                .get_bucket_info(&mount_config.bucket_id)
                .await
                .ok()
                .flatten()
                .map(|info| info.name)
                .unwrap_or_else(|| "jax".to_string());

            vec![
                fuser::MountOption::FSName("jax".to_string()),
                fuser::MountOption::AutoUnmount,
                fuser::MountOption::CUSTOM(format!("volname={}", bucket_name)),
                fuser::MountOption::CUSTOM("local".to_string()),
                fuser::MountOption::CUSTOM("noappledouble".to_string()),
            ]
        };

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

        // Wrap session in a handle for thread safety
        let session_handle = SessionHandle::spawn(session);

        // Create live mount
        let live_mount = LiveMount {
            mount: mount_arc,
            session: Some(session_handle),
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
                // Signal the session to unmount
                if let Some(ref mut session) = live.session {
                    session.unmount();
                }
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

    /// Spawn a background task to handle save requests from FUSE
    fn spawn_save_handler(
        &self,
        mut save_rx: mpsc::Receiver<SaveRequest>,
        mount: Arc<RwLock<Mount>>,
    ) {
        let peer = self.peer.clone();

        tokio::spawn(async move {
            while let Some(request) = save_rx.recv().await {
                tracing::debug!("Received save request for mount {}", request.mount_id);

                // Get the current mount state and save it
                let mount_guard = mount.read().await;
                match peer.save_mount(&mount_guard, false).await {
                    Ok(link) => {
                        tracing::info!(
                            "Successfully saved mount {} to {}",
                            request.mount_id,
                            link.hash()
                        );
                    }
                    Err(e) => {
                        tracing::error!("Failed to save mount {}: {}", request.mount_id, e);
                    }
                }
            }

            tracing::debug!("Save handler shutting down");
        });
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
