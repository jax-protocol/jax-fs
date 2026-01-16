//! Mount Manager - handles FUSE mount lifecycle
//!
//! Responsibilities:
//! - Track active FUSE mount processes
//! - Spawn/kill mount processes
//! - Update mount status in database
//! - Handle auto-mount on startup
//! - Clean unmount on shutdown

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use uuid::Uuid;

use super::database::mount_queries::{MountInfo, MountStatus};
use super::database::Database;

/// Active mount process info
#[allow(dead_code)]
struct ActiveMount {
    mount_id: Uuid,
    bucket_id: Uuid,
    mount_point: PathBuf,
    process: Child,
}

/// Mount manager handles the lifecycle of FUSE mounts
pub struct MountManager {
    database: Database,
    /// Currently running mount processes
    active_mounts: RwLock<HashMap<Uuid, ActiveMount>>,
    /// Path to the jax executable for spawning mounts
    jax_executable: PathBuf,
    /// API endpoint for the daemon
    api_endpoint: String,
}

#[derive(Debug, thiserror::Error)]
pub enum MountError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Mount not found: {0}")]
    NotFound(Uuid),
    #[error("Mount already running: {0}")]
    AlreadyRunning(Uuid),
    #[error("Mount not running: {0}")]
    NotRunning(Uuid),
    #[error("Failed to spawn mount process: {0}")]
    SpawnFailed(String),
    #[error("Failed to stop mount: {0}")]
    StopFailed(String),
    #[error("Mount point does not exist: {0}")]
    MountPointNotFound(PathBuf),
    #[error("Bucket not found: {0}")]
    BucketNotFound(Uuid),
}

impl MountManager {
    /// Create a new mount manager
    pub fn new(database: Database, api_endpoint: String) -> Self {
        // Find the jax executable - use current exe path
        let jax_executable = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("jax"));

        Self {
            database,
            active_mounts: RwLock::new(HashMap::new()),
            jax_executable,
            api_endpoint,
        }
    }

    /// Start a configured mount
    pub async fn start_mount(&self, mount_id: &Uuid) -> Result<(), MountError> {
        // Get mount config from database
        let mount = self
            .database
            .get_mount(mount_id)
            .await?
            .ok_or(MountError::NotFound(*mount_id))?;

        // Check if already running
        {
            let active = self.active_mounts.read().await;
            if active.contains_key(mount_id) {
                return Err(MountError::AlreadyRunning(*mount_id));
            }
        }

        // Verify mount point exists
        let mount_point = PathBuf::from(&mount.mount_point);
        if !mount_point.exists() {
            // Try to create it
            if let Err(e) = std::fs::create_dir_all(&mount_point) {
                tracing::warn!("Failed to create mount point {}: {}", mount.mount_point, e);
                return Err(MountError::MountPointNotFound(mount_point));
            }
        }

        // Spawn the mount process
        let child = self.spawn_mount_process(&mount).await?;
        let pid = child.id().map(|p| p as i64);

        // Track the active mount
        {
            let mut active = self.active_mounts.write().await;
            active.insert(
                *mount_id,
                ActiveMount {
                    mount_id: *mount_id,
                    bucket_id: mount.bucket_id,
                    mount_point,
                    process: child,
                },
            );
        }

        // Update database status
        self.database
            .update_mount_status(mount_id, MountStatus::Running, pid, None)
            .await?;

        tracing::info!(
            "Started mount {} for bucket {} at {}",
            mount_id,
            mount.bucket_id,
            mount.mount_point
        );

        Ok(())
    }

    /// Stop a running mount
    pub async fn stop_mount(&self, mount_id: &Uuid) -> Result<(), MountError> {
        // Get and remove from active mounts
        let mut active_mount = {
            let mut active = self.active_mounts.write().await;
            active
                .remove(mount_id)
                .ok_or(MountError::NotRunning(*mount_id))?
        };

        // Try graceful unmount first using fusermount/umount
        let unmount_result = self.unmount_path(&active_mount.mount_point).await;

        // Kill the process if still running
        if let Err(e) = active_mount.process.kill().await {
            tracing::warn!("Failed to kill mount process: {}", e);
        }

        // Wait for process to exit
        let _ = tokio::time::timeout(Duration::from_secs(5), active_mount.process.wait()).await;

        // Update database status
        if let Err(unmount_err) = unmount_result {
            self.database
                .update_mount_status(
                    mount_id,
                    MountStatus::Error,
                    None,
                    Some(&unmount_err.to_string()),
                )
                .await?;
        } else {
            self.database
                .update_mount_status(mount_id, MountStatus::Stopped, None, None)
                .await?;
        }

        tracing::info!(
            "Stopped mount {} at {}",
            mount_id,
            active_mount.mount_point.display()
        );

        Ok(())
    }

    /// Spawn a FUSE mount process for the given config
    async fn spawn_mount_process(&self, mount: &MountInfo) -> Result<Child, MountError> {
        let mut cmd = Command::new(&self.jax_executable);

        cmd.arg("bucket")
            .arg("fuse")
            .arg(mount.bucket_id.to_string())
            .arg(&mount.mount_point)
            .arg("--cache-size-mb")
            .arg(mount.cache_size_mb.to_string())
            .arg("--cache-ttl-secs")
            .arg(mount.cache_ttl_secs.to_string());

        if mount.read_only {
            cmd.arg("--read-only");
        }

        // Set API endpoint environment variable
        cmd.env("JAX_API_ENDPOINT", &self.api_endpoint);

        // Don't daemonize - we manage the process
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        let child = cmd
            .spawn()
            .map_err(|e| MountError::SpawnFailed(e.to_string()))?;

        Ok(child)
    }

    /// Unmount a path using platform-specific command
    async fn unmount_path(&self, mount_point: &PathBuf) -> Result<(), MountError> {
        #[cfg(target_os = "linux")]
        let result = Command::new("fusermount")
            .args(["-u", &mount_point.to_string_lossy()])
            .output()
            .await;

        #[cfg(target_os = "macos")]
        let result = Command::new("umount")
            .arg(mount_point)
            .output()
            .await;

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        let result: Result<std::process::Output, std::io::Error> =
            Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "Unsupported platform"));

        match result {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => Err(MountError::StopFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            )),
            Err(e) => Err(MountError::StopFailed(e.to_string())),
        }
    }

    /// Start all mounts configured for auto-mount
    /// This is called on daemon startup and should not block
    pub async fn start_auto_mounts(&self) {
        match self.database.list_auto_mounts().await {
            Ok(mounts) => {
                for mount in mounts {
                    let mount_id = mount.mount_id;
                    match self.start_mount(&mount_id).await {
                        Ok(_) => {
                            tracing::info!(
                                "Auto-mounted {} at {}",
                                mount.bucket_id,
                                mount.mount_point
                            );
                        }
                        Err(e) => {
                            tracing::error!(
                                "Failed to auto-mount {} at {}: {}",
                                mount.bucket_id,
                                mount.mount_point,
                                e
                            );
                            // Update status to error but don't fail startup
                            let _ = self
                                .database
                                .update_mount_status(
                                    &mount_id,
                                    MountStatus::Error,
                                    None,
                                    Some(&e.to_string()),
                                )
                                .await;
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to list auto-mounts: {}", e);
            }
        }
    }

    /// Stop all running mounts gracefully
    /// This is called on daemon shutdown
    pub async fn stop_all_mounts(&self) {
        let mount_ids: Vec<Uuid> = {
            let active = self.active_mounts.read().await;
            active.keys().copied().collect()
        };

        for mount_id in mount_ids {
            if let Err(e) = self.stop_mount(&mount_id).await {
                tracing::error!("Failed to stop mount {}: {}", mount_id, e);
            }
        }
    }

    /// Get list of currently running mount IDs
    pub async fn list_running(&self) -> Vec<Uuid> {
        let active = self.active_mounts.read().await;
        active.keys().copied().collect()
    }

    /// Check if a mount is currently running
    pub async fn is_running(&self, mount_id: &Uuid) -> bool {
        let active = self.active_mounts.read().await;
        active.contains_key(mount_id)
    }

    /// Check health of running mounts and restart crashed ones if auto-mount enabled
    pub async fn health_check(&self) {
        let mut to_restart = Vec::new();
        let mut to_remove = Vec::new();

        // Check each active mount
        {
            let mut active = self.active_mounts.write().await;
            for (mount_id, active_mount) in active.iter_mut() {
                match active_mount.process.try_wait() {
                    Ok(Some(status)) => {
                        // Process has exited
                        tracing::warn!(
                            "Mount {} process exited with status: {:?}",
                            mount_id,
                            status
                        );
                        to_remove.push(*mount_id);

                        // Check if auto-mount is enabled for restart
                        if let Ok(Some(mount)) = self.database.get_mount(mount_id).await {
                            if mount.auto_mount && mount.enabled {
                                to_restart.push(*mount_id);
                            }
                        }
                    }
                    Ok(None) => {
                        // Process still running, good
                    }
                    Err(e) => {
                        tracing::error!("Failed to check mount {} process status: {}", mount_id, e);
                    }
                }
            }

            // Remove exited mounts from active
            for mount_id in &to_remove {
                active.remove(mount_id);
            }
        }

        // Update database for removed mounts
        for mount_id in &to_remove {
            let _ = self
                .database
                .update_mount_status(mount_id, MountStatus::Stopped, None, None)
                .await;
        }

        // Restart crashed mounts with auto-mount enabled
        for mount_id in to_restart {
            tracing::info!("Restarting crashed mount {}", mount_id);
            if let Err(e) = self.start_mount(&mount_id).await {
                tracing::error!("Failed to restart mount {}: {}", mount_id, e);
            }
        }
    }
}
