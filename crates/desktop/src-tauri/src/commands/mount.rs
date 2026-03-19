//! FUSE mount IPC commands
//!
//! These commands use the daemon's HTTP API via ApiClient for mount management,
//! enabling both embedded and sidecar daemon modes.
//! The `is_fuse_available` and `mount_bucket`/`unmount_bucket` commands
//! perform local checks and orchestrate multiple API calls.

use serde::{Deserialize, Serialize};
use tauri::State;

#[cfg(feature = "fuse")]
use uuid::Uuid;

#[cfg(feature = "fuse")]
use jax_daemon::http_server::api::client::ApiClient;
#[cfg(feature = "fuse")]
use jax_daemon::http_server::api::v0::bucket::list::ListRequest;
#[cfg(feature = "fuse")]
use jax_daemon::http_server::api::v0::mounts::{
    CreateMountRequest, DeleteMountRequest, GetMountRequest, ListMountsRequest,
    MountInfo as DaemonMountInfo, StartMountRequest, StopMountRequest, UpdateMountBody,
    UpdateMountRequest,
};

use crate::AppState;

/// Get the platform-specific base mount directory
#[cfg(feature = "fuse")]
fn get_mount_base_dir() -> std::path::PathBuf {
    #[cfg(target_os = "macos")]
    {
        std::path::PathBuf::from("/Volumes")
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(user) = std::env::var("USER") {
            let media_path = std::path::PathBuf::from(format!("/media/{}", user));
            if media_path.exists() {
                return media_path;
            }
            let run_media_path = std::path::PathBuf::from(format!("/run/media/{}", user));
            if run_media_path.exists() {
                return run_media_path;
            }
            media_path
        } else {
            std::path::PathBuf::from("/media")
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        std::path::PathBuf::from("/mnt")
    }
}

/// Generate a unique mount point for a bucket, handling naming conflicts
#[cfg(feature = "fuse")]
fn generate_mount_point(bucket_name: &str, existing_mounts: &[String]) -> std::path::PathBuf {
    let base_dir = get_mount_base_dir();
    let sanitized_name = sanitize_mount_name(bucket_name);

    let base_path = base_dir.join(&sanitized_name);
    if !existing_mounts.contains(&base_path.to_string_lossy().to_string()) && !base_path.exists() {
        return base_path;
    }

    for i in 2..100 {
        let numbered_path = base_dir.join(format!("{}-{}", sanitized_name, i));
        if !existing_mounts.contains(&numbered_path.to_string_lossy().to_string())
            && !numbered_path.exists()
        {
            return numbered_path;
        }
    }

    let uuid_suffix = Uuid::new_v4().to_string()[..8].to_string();
    base_dir.join(format!("{}-{}", sanitized_name, uuid_suffix))
}

/// Sanitize bucket name for use as mount point
#[cfg(feature = "fuse")]
fn sanitize_mount_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Create mount point directory with platform-specific privilege escalation
#[cfg(feature = "fuse")]
async fn create_mount_point(path: &std::path::Path) -> Result<(), String> {
    if std::fs::create_dir_all(path).is_ok() {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            r#"do shell script "mkdir -p '{}'" with administrator privileges"#,
            path.display()
        );

        let output = std::process::Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|e| format!("Failed to run osascript: {}", e))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("Failed to create mount point: {}", stderr))
        }
    }

    #[cfg(target_os = "linux")]
    {
        let output = std::process::Command::new("pkexec")
            .args(["mkdir", "-p", &path.to_string_lossy()])
            .output()
            .map_err(|e| format!("Failed to run pkexec: {}", e))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("Failed to create mount point: {}", stderr))
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err("Unsupported platform for privilege escalation".to_string())
    }
}

/// Mount information for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountInfo {
    pub mount_id: String,
    pub bucket_id: String,
    pub mount_point: String,
    pub enabled: bool,
    pub auto_mount: bool,
    pub read_only: bool,
    pub cache_size_mb: u32,
    pub cache_ttl_secs: u32,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[cfg(feature = "fuse")]
impl From<DaemonMountInfo> for MountInfo {
    fn from(m: DaemonMountInfo) -> Self {
        Self {
            mount_id: m.mount_id.to_string(),
            bucket_id: m.bucket_id.to_string(),
            mount_point: m.mount_point,
            enabled: m.enabled,
            auto_mount: m.auto_mount,
            read_only: m.read_only,
            cache_size_mb: m.cache_size_mb,
            cache_ttl_secs: m.cache_ttl_secs,
            status: m.status,
            error_message: m.error_message,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

/// Request to create a new mount (from frontend)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopCreateMountRequest {
    pub bucket_id: String,
    pub mount_point: String,
    #[serde(default)]
    pub auto_mount: bool,
    #[serde(default)]
    pub read_only: bool,
    pub cache_size_mb: Option<u32>,
    pub cache_ttl_secs: Option<u32>,
}

/// Request to update a mount (from frontend)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopUpdateMountRequest {
    pub mount_point: Option<String>,
    pub enabled: Option<bool>,
    pub auto_mount: Option<bool>,
    pub read_only: Option<bool>,
    pub cache_size_mb: Option<u32>,
    pub cache_ttl_secs: Option<u32>,
}

/// Get an ApiClient from the app state.
#[cfg(feature = "fuse")]
async fn get_client(state: &State<'_, AppState>) -> Result<ApiClient, String> {
    let inner = state.inner.read().await;
    let inner = inner.as_ref().ok_or("Daemon not connected")?;
    Ok(inner.client.clone())
}

/// List all mounts
#[tauri::command]
#[cfg(feature = "fuse")]
pub async fn list_mounts(state: State<'_, AppState>) -> Result<Vec<MountInfo>, String> {
    let mut client = get_client(&state).await?;
    let resp = client
        .call(ListMountsRequest {})
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.mounts.into_iter().map(Into::into).collect())
}

#[tauri::command]
#[cfg(not(feature = "fuse"))]
pub async fn list_mounts(_state: State<'_, AppState>) -> Result<Vec<MountInfo>, String> {
    Ok(Vec::new())
}

/// Create a new mount
#[tauri::command]
#[cfg(feature = "fuse")]
pub async fn create_mount(
    state: State<'_, AppState>,
    request: DesktopCreateMountRequest,
) -> Result<MountInfo, String> {
    let mut client = get_client(&state).await?;
    let bucket_id =
        Uuid::parse_str(&request.bucket_id).map_err(|e| format!("Invalid bucket ID: {}", e))?;

    let resp = client
        .call(CreateMountRequest {
            bucket_id,
            mount_point: request.mount_point,
            auto_mount: request.auto_mount,
            read_only: request.read_only,
            cache_size_mb: request.cache_size_mb,
            cache_ttl_secs: request.cache_ttl_secs,
        })
        .await
        .map_err(|e| e.to_string())?;

    Ok(resp.mount.into())
}

#[tauri::command]
#[cfg(not(feature = "fuse"))]
pub async fn create_mount(
    _state: State<'_, AppState>,
    _request: DesktopCreateMountRequest,
) -> Result<MountInfo, String> {
    Err("FUSE support not enabled".to_string())
}

/// Get a mount by ID
#[tauri::command]
#[cfg(feature = "fuse")]
pub async fn get_mount(state: State<'_, AppState>, mount_id: String) -> Result<MountInfo, String> {
    let mut client = get_client(&state).await?;
    let id = Uuid::parse_str(&mount_id).map_err(|e| format!("Invalid mount ID: {}", e))?;
    let resp = client
        .call(GetMountRequest { mount_id: id })
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.mount.into())
}

#[tauri::command]
#[cfg(not(feature = "fuse"))]
pub async fn get_mount(
    _state: State<'_, AppState>,
    _mount_id: String,
) -> Result<MountInfo, String> {
    Err("FUSE support not enabled".to_string())
}

/// Update a mount
#[tauri::command]
#[cfg(feature = "fuse")]
pub async fn update_mount(
    state: State<'_, AppState>,
    mount_id: String,
    request: DesktopUpdateMountRequest,
) -> Result<MountInfo, String> {
    let mut client = get_client(&state).await?;
    let id = Uuid::parse_str(&mount_id).map_err(|e| format!("Invalid mount ID: {}", e))?;
    let resp = client
        .call(UpdateMountRequest {
            mount_id: id,
            body: UpdateMountBody {
                mount_point: request.mount_point,
                enabled: request.enabled,
                auto_mount: request.auto_mount,
                read_only: request.read_only,
                cache_size_mb: request.cache_size_mb,
                cache_ttl_secs: request.cache_ttl_secs,
            },
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.mount.into())
}

#[tauri::command]
#[cfg(not(feature = "fuse"))]
pub async fn update_mount(
    _state: State<'_, AppState>,
    _mount_id: String,
    _request: DesktopUpdateMountRequest,
) -> Result<MountInfo, String> {
    Err("FUSE support not enabled".to_string())
}

/// Delete a mount
#[tauri::command]
#[cfg(feature = "fuse")]
pub async fn delete_mount(state: State<'_, AppState>, mount_id: String) -> Result<bool, String> {
    let mut client = get_client(&state).await?;
    let id = Uuid::parse_str(&mount_id).map_err(|e| format!("Invalid mount ID: {}", e))?;
    let resp = client
        .call(DeleteMountRequest { mount_id: id })
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.deleted)
}

#[tauri::command]
#[cfg(not(feature = "fuse"))]
pub async fn delete_mount(_state: State<'_, AppState>, _mount_id: String) -> Result<bool, String> {
    Err("FUSE support not enabled".to_string())
}

/// Start a mount (spawn FUSE process)
#[tauri::command]
#[cfg(feature = "fuse")]
pub async fn start_mount(state: State<'_, AppState>, mount_id: String) -> Result<bool, String> {
    let mut client = get_client(&state).await?;
    let id = Uuid::parse_str(&mount_id).map_err(|e| format!("Invalid mount ID: {}", e))?;
    let resp = client
        .call(StartMountRequest { mount_id: id })
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.started)
}

#[tauri::command]
#[cfg(not(feature = "fuse"))]
pub async fn start_mount(_state: State<'_, AppState>, _mount_id: String) -> Result<bool, String> {
    Err("FUSE support not enabled".to_string())
}

/// Stop a mount
#[tauri::command]
#[cfg(feature = "fuse")]
pub async fn stop_mount(state: State<'_, AppState>, mount_id: String) -> Result<bool, String> {
    let mut client = get_client(&state).await?;
    let id = Uuid::parse_str(&mount_id).map_err(|e| format!("Invalid mount ID: {}", e))?;
    let resp = client
        .call(StopMountRequest { mount_id: id })
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.stopped)
}

#[tauri::command]
#[cfg(not(feature = "fuse"))]
pub async fn stop_mount(_state: State<'_, AppState>, _mount_id: String) -> Result<bool, String> {
    Err("FUSE support not enabled".to_string())
}

/// Check if FUSE support is available (local system + daemon capability)
#[tauri::command]
pub async fn is_fuse_available(state: State<'_, AppState>) -> Result<bool, String> {
    #[cfg(not(feature = "fuse"))]
    {
        let _ = state; // suppress unused warning
        Ok(false)
    }

    #[cfg(feature = "fuse")]
    {
        // Check local system first
        if !check_fuse_installed() {
            return Ok(false);
        }

        // Check if daemon has FUSE feature enabled
        let inner = state.inner.read().await;
        match inner.as_ref() {
            Some(daemon) => Ok(daemon
                .build_info
                .as_ref()
                .is_some_and(|b| b.has_feature("fuse"))),
            None => Ok(false),
        }
    }
}

#[cfg(feature = "fuse")]
fn check_fuse_installed() -> bool {
    #[cfg(target_os = "macos")]
    {
        std::path::Path::new("/Library/Filesystems/macfuse.fs").exists()
            || std::path::Path::new("/Library/Filesystems/osxfuse.fs").exists()
    }

    #[cfg(target_os = "linux")]
    {
        std::path::Path::new("/dev/fuse").exists()
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        false
    }
}

/// Mount a bucket with automatic mount point selection (desktop app simplified API)
#[tauri::command]
#[cfg(feature = "fuse")]
pub async fn mount_bucket(
    state: State<'_, AppState>,
    bucket_id: String,
) -> Result<MountInfo, String> {
    let mut client = get_client(&state).await?;
    let bucket_uuid =
        Uuid::parse_str(&bucket_id).map_err(|e| format!("Invalid bucket ID: {}", e))?;

    // Get bucket name via list endpoint
    let list_resp = client
        .call(ListRequest {
            prefix: None,
            limit: None,
            status: None,
        })
        .await
        .map_err(|e| e.to_string())?;

    let bucket_name = list_resp
        .buckets
        .into_iter()
        .find(|b| b.bucket_id == bucket_uuid)
        .map(|b| b.name)
        .unwrap_or_else(|| "bucket".to_string());

    // Get existing mounts to check for conflicts
    let mounts_resp = client
        .call(ListMountsRequest {})
        .await
        .map_err(|e| e.to_string())?;

    let existing_mounts: Vec<String> = mounts_resp
        .mounts
        .iter()
        .map(|m| m.mount_point.clone())
        .collect();

    // Generate unique mount point
    let mount_point = generate_mount_point(&bucket_name, &existing_mounts);
    let mount_point_str = mount_point.to_string_lossy().to_string();

    // Create mount point directory
    create_mount_point(&mount_point).await?;

    // Create mount via API
    let create_resp = client
        .call(CreateMountRequest {
            bucket_id: bucket_uuid,
            mount_point: mount_point_str,
            auto_mount: false,
            read_only: false,
            cache_size_mb: None,
            cache_ttl_secs: None,
        })
        .await
        .map_err(|e| e.to_string())?;

    let mount_id = create_resp.mount.mount_id;

    // Start the mount
    client
        .call(StartMountRequest { mount_id })
        .await
        .map_err(|e| e.to_string())?;

    // Get updated mount info
    let get_resp = client
        .call(GetMountRequest { mount_id })
        .await
        .map_err(|e| e.to_string())?;

    Ok(get_resp.mount.into())
}

#[tauri::command]
#[cfg(not(feature = "fuse"))]
pub async fn mount_bucket(
    _state: State<'_, AppState>,
    _bucket_id: String,
) -> Result<MountInfo, String> {
    Err("FUSE support not enabled".to_string())
}

/// Unmount a bucket by bucket ID (finds and stops the mount)
#[tauri::command]
#[cfg(feature = "fuse")]
pub async fn unmount_bucket(state: State<'_, AppState>, bucket_id: String) -> Result<bool, String> {
    let mut client = get_client(&state).await?;
    let bucket_uuid =
        Uuid::parse_str(&bucket_id).map_err(|e| format!("Invalid bucket ID: {}", e))?;

    // List mounts to find the one for this bucket
    let mounts_resp = client
        .call(ListMountsRequest {})
        .await
        .map_err(|e| e.to_string())?;

    let mount = mounts_resp
        .mounts
        .into_iter()
        .find(|m| m.bucket_id == bucket_uuid)
        .ok_or("No mount found for this bucket")?;

    // Stop the mount
    client
        .call(StopMountRequest {
            mount_id: mount.mount_id,
        })
        .await
        .map_err(|e| e.to_string())?;

    // Delete the mount config
    client
        .call(DeleteMountRequest {
            mount_id: mount.mount_id,
        })
        .await
        .map_err(|e| e.to_string())?;

    Ok(true)
}

#[tauri::command]
#[cfg(not(feature = "fuse"))]
pub async fn unmount_bucket(
    _state: State<'_, AppState>,
    _bucket_id: String,
) -> Result<bool, String> {
    Err("FUSE support not enabled".to_string())
}

/// Check if a bucket is currently mounted
#[tauri::command]
#[cfg(feature = "fuse")]
pub async fn is_bucket_mounted(
    state: State<'_, AppState>,
    bucket_id: String,
) -> Result<Option<MountInfo>, String> {
    let mut client = get_client(&state).await?;
    let bucket_uuid =
        Uuid::parse_str(&bucket_id).map_err(|e| format!("Invalid bucket ID: {}", e))?;

    let resp = client
        .call(ListMountsRequest {})
        .await
        .map_err(|e| e.to_string())?;

    let mount = resp
        .mounts
        .into_iter()
        .find(|m| m.bucket_id == bucket_uuid && m.status == "running");

    Ok(mount.map(Into::into))
}

#[tauri::command]
#[cfg(not(feature = "fuse"))]
pub async fn is_bucket_mounted(
    _state: State<'_, AppState>,
    _bucket_id: String,
) -> Result<Option<MountInfo>, String> {
    Ok(None)
}
