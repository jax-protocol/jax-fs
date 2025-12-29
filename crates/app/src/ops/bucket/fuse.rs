//! FUSE mount command for jax-bucket
//!
//! Mounts a bucket as a local FUSE filesystem.

use clap::Args;
use fuser::MountOption;
use std::fs::File;
use std::path::PathBuf;
use uuid::Uuid;

use crate::daemon::http_server::api::client::ApiError;
use crate::fuse::{CacheConfig, JaxFs};
use crate::op::Op;

#[derive(Args, Debug, Clone)]
pub struct Fuse {
    /// Bucket ID (or use --name)
    #[arg(long, group = "bucket_identifier")]
    pub bucket_id: Option<Uuid>,

    /// Bucket name (or use --bucket-id)
    #[arg(long, group = "bucket_identifier")]
    pub name: Option<String>,

    /// Local directory to mount the bucket at
    #[arg(long)]
    pub mount_point: PathBuf,

    /// Allow other users to access the mount (requires user_allow_other in /etc/fuse.conf)
    #[arg(long, default_value = "false")]
    pub allow_other: bool,

    /// Run in read-only mode
    #[arg(long, default_value = "false")]
    pub read_only: bool,

    /// Run mount in background (daemonize)
    #[arg(long, short = 'd')]
    pub daemon: bool,

    /// Unmount an existing FUSE mount instead of mounting
    #[arg(long, short = 'u')]
    pub unmount: bool,

    /// PID file path for daemon mode (default: ~/.jax/fuse-{mount}.pid)
    #[arg(long)]
    pub pid_file: Option<PathBuf>,

    /// Log file path for daemon mode (default: ~/.jax/fuse-{mount}.log)
    #[arg(long)]
    pub log_file: Option<PathBuf>,

    /// Cache max size in MB (default: 100)
    #[arg(long, default_value = "100")]
    pub cache_size_mb: u64,

    /// Cache TTL in seconds (default: 60)
    #[arg(long, default_value = "60")]
    pub cache_ttl_secs: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum FuseError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Either --bucket-id or --name must be provided")]
    NoBucketIdentifier,
    #[error("Mount point does not exist: {0}")]
    MountPointNotFound(PathBuf),
    #[error("Mount point is not a directory: {0}")]
    MountPointNotDirectory(PathBuf),
    #[error("FUSE error: {0}")]
    Fuse(String),
    #[error("Daemon error: {0}")]
    Daemon(String),
    #[error("Unmount failed: {0}")]
    Unmount(String),
}

impl Fuse {
    /// Perform unmount operation
    fn do_unmount(&self) -> Result<String, FuseError> {
        #[cfg(target_os = "linux")]
        {
            let status = std::process::Command::new("fusermount")
                .args(["-u", &self.mount_point.to_string_lossy()])
                .status()
                .map_err(|e| FuseError::Unmount(e.to_string()))?;

            if !status.success() {
                return Err(FuseError::Unmount(format!(
                    "fusermount -u failed with status: {}",
                    status
                )));
            }
        }

        #[cfg(target_os = "macos")]
        {
            let status = std::process::Command::new("umount")
                .arg(&self.mount_point)
                .status()
                .map_err(|e| FuseError::Unmount(e.to_string()))?;

            if !status.success() {
                return Err(FuseError::Unmount(format!(
                    "umount failed with status: {}",
                    status
                )));
            }
        }

        Ok(format!("Unmounted {}", self.mount_point.display()))
    }

    /// Get default jax directory
    fn jax_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".jax")
    }

    /// Get PID file path for this mount
    fn get_pid_file(&self) -> PathBuf {
        self.pid_file.clone().unwrap_or_else(|| {
            let mount_name = self
                .mount_point
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("mount");
            Self::jax_dir().join(format!("fuse-{}.pid", mount_name))
        })
    }

    /// Get log file path for this mount
    fn get_log_file(&self) -> PathBuf {
        self.log_file.clone().unwrap_or_else(|| {
            let mount_name = self
                .mount_point
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("mount");
            Self::jax_dir().join(format!("fuse-{}.log", mount_name))
        })
    }
}

#[async_trait::async_trait]
impl Op for Fuse {
    type Error = FuseError;
    type Output = String;

    async fn execute(&self, ctx: &crate::op::OpContext) -> Result<Self::Output, Self::Error> {
        // Handle unmount
        if self.unmount {
            return self.do_unmount();
        }

        let mut client = ctx.client.clone();

        // Resolve bucket name to UUID if needed
        let bucket_id = if let Some(id) = self.bucket_id {
            id
        } else if let Some(ref name) = self.name {
            client.resolve_bucket_name(name).await?
        } else {
            return Err(FuseError::NoBucketIdentifier);
        };

        // Create mount point if it doesn't exist
        if !self.mount_point.exists() {
            std::fs::create_dir_all(&self.mount_point)?;
        }

        // Verify mount point is a directory
        if !self.mount_point.is_dir() {
            return Err(FuseError::MountPointNotDirectory(self.mount_point.clone()));
        }

        // Get the tokio runtime handle
        let rt = tokio::runtime::Handle::current();

        // Create cache config
        let cache_config = CacheConfig {
            max_bytes: self.cache_size_mb * 1024 * 1024,
            ttl_secs: self.cache_ttl_secs,
        };

        // Create the FUSE filesystem with cache
        let fs = JaxFs::with_cache_config(rt, ctx.client.clone(), bucket_id, cache_config);

        // Build mount options
        let mut options = vec![
            MountOption::FSName("jax".to_string()),
            MountOption::AutoUnmount,
            MountOption::DefaultPermissions,
        ];

        if self.allow_other {
            options.push(MountOption::AllowOther);
        }

        if self.read_only {
            options.push(MountOption::RO);
        }

        let mount_point = self.mount_point.clone();

        // Handle daemon mode
        if self.daemon {
            // Ensure jax directory exists
            let jax_dir = Self::jax_dir();
            std::fs::create_dir_all(&jax_dir)?;

            let pid_file = self.get_pid_file();
            let log_file = self.get_log_file();

            println!(
                "Daemonizing FUSE mount for bucket {} at {}",
                bucket_id,
                self.mount_point.display()
            );
            println!("PID file: {}", pid_file.display());
            println!("Log file: {}", log_file.display());

            // Open log file for stdout/stderr
            let log = File::create(&log_file)?;
            let log_err = log.try_clone()?;

            let daemonize = daemonize::Daemonize::new()
                .pid_file(&pid_file)
                .stdout(log)
                .stderr(log_err)
                .working_directory(".");

            match daemonize.start() {
                Ok(_) => {
                    // We're now the daemon child process
                    // Run the FUSE mount (blocking)
                    if let Err(e) = fuser::mount2(fs, &mount_point, &options) {
                        eprintln!("FUSE mount error: {}", e);
                        std::process::exit(1);
                    }
                    // If we get here, the mount was unmounted cleanly
                    std::process::exit(0);
                }
                Err(e) => {
                    return Err(FuseError::Daemon(e.to_string()));
                }
            }
        }

        // Foreground mode
        println!(
            "Mounting bucket {} at {}",
            bucket_id,
            self.mount_point.display()
        );
        println!("Press Ctrl+C to unmount");
        println!();

        // Run the FUSE mount in a blocking thread
        let result = tokio::task::spawn_blocking(move || fuser::mount2(fs, &mount_point, &options))
            .await
            .map_err(|e| FuseError::Fuse(e.to_string()))?;

        result.map_err(FuseError::Io)?;

        Ok(format!("Unmounted {}", self.mount_point.display()))
    }
}
