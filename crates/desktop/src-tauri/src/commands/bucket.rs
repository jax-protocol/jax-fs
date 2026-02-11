//! Bucket IPC commands
//!
//! These commands access ServiceState directly for bucket operations.
//! Commands that need the full API flow (create, share, ping) still use HTTP.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::State;
use time::OffsetDateTime;
use uuid::Uuid;

use common::linked_data::{Hash, Link};
use common::mount::Mount;
use jax_daemon::ServiceState;

use crate::AppState;

/// Bucket information returned to the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketInfo {
    pub bucket_id: Uuid,
    pub name: String,
    pub link_hash: String,
    pub height: u64,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

/// File/directory entry returned by ls command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub mime_type: String,
    pub link_hash: String,
}

/// Result of reading a file with cat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatResult {
    pub content: Vec<u8>,
    pub mime_type: String,
    pub size: usize,
}

/// History entry for version list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub link_hash: String,
    pub height: u64,
    pub published: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

/// Get the daemon API base URL (for commands that still use HTTP)
async fn get_daemon_url(state: &State<'_, AppState>) -> Result<String, String> {
    let inner = state.inner.read().await;
    let inner = inner.as_ref().ok_or("Daemon not started")?;
    Ok(format!("http://localhost:{}", inner.api_port))
}

/// Get the ServiceState from AppState
async fn get_service(state: &State<'_, AppState>) -> Result<ServiceState, String> {
    let inner = state.inner.read().await;
    let inner = inner.as_ref().ok_or("Daemon not started")?;
    Ok(inner.service.clone())
}

/// Parse a bucket_id string into Uuid
fn parse_bucket_id(bucket_id: &str) -> Result<Uuid, String> {
    bucket_id
        .parse()
        .map_err(|e| format!("Invalid bucket ID: {}", e))
}

/// Extract MIME type from a NodeLink
fn node_link_mime(node_link: &common::mount::NodeLink) -> String {
    if node_link.is_dir() {
        "inode/directory".to_string()
    } else {
        node_link
            .data()
            .and_then(|data| data.mime())
            .map(|mime| mime.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string())
    }
}

/// List all buckets
#[tauri::command]
pub async fn list_buckets(state: State<'_, AppState>) -> Result<Vec<BucketInfo>, String> {
    let service = get_service(&state).await?;
    let db = service.database();

    let buckets = db
        .list_buckets(None, None)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    Ok(buckets
        .into_iter()
        .map(|b| BucketInfo {
            bucket_id: b.id,
            name: b.name,
            link_hash: b.link.to_string(),
            height: 0, // list_buckets doesn't include height
            created_at: b.created_at,
        })
        .collect())
}

/// Create a new bucket (still uses HTTP — create needs full API flow with init+save)
#[tauri::command]
pub async fn create_bucket(state: State<'_, AppState>, name: String) -> Result<BucketInfo, String> {
    let base_url = get_daemon_url(&state).await?;
    let url = format!("{}/api/v0/bucket", base_url);

    #[derive(Serialize)]
    struct CreateRequest {
        name: String,
    }

    #[derive(Deserialize)]
    struct CreateResponse {
        bucket_id: Uuid,
        name: String,
        #[serde(with = "time::serde::rfc3339")]
        created_at: OffsetDateTime,
    }

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&CreateRequest { name: name.clone() })
        .send()
        .await
        .map_err(|e| format!("Failed to connect to daemon: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Failed to create bucket ({}): {}", status, body));
    }

    let create_response: CreateResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(BucketInfo {
        bucket_id: create_response.bucket_id,
        name: create_response.name,
        link_hash: String::new(),
        height: 0,
        created_at: create_response.created_at,
    })
}

/// Delete a file or directory (or entire bucket at path "/")
#[tauri::command]
pub async fn delete_bucket(state: State<'_, AppState>, bucket_id: String) -> Result<(), String> {
    let service = get_service(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;

    let mut mount = service
        .peer()
        .mount(bucket_uuid)
        .await
        .map_err(|e| e.to_string())?;

    mount
        .rm(&PathBuf::from("/"))
        .await
        .map_err(|e| e.to_string())?;

    service
        .peer()
        .save_mount(&mount, false)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// List directory contents
#[tauri::command]
pub async fn ls(
    state: State<'_, AppState>,
    bucket_id: String,
    path: String,
) -> Result<Vec<FileEntry>, String> {
    let service = get_service(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;

    let mount = service
        .peer()
        .mount_for_read(bucket_uuid)
        .await
        .map_err(|e| e.to_string())?;

    let items = mount
        .ls(&PathBuf::from(&path))
        .await
        .map_err(|e| e.to_string())?;

    Ok(items
        .into_iter()
        .map(|(entry_path, node_link)| {
            let name = entry_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let mime_type = node_link_mime(&node_link);
            let link_hash = node_link.link().to_string();
            let is_dir = node_link.is_dir();

            // Build full path
            let full_path = if path == "/" {
                format!("/{}", name)
            } else {
                format!("{}/{}", path.trim_end_matches('/'), name)
            };

            FileEntry {
                path: full_path,
                name,
                is_dir,
                mime_type,
                link_hash,
            }
        })
        .collect())
}

/// Read file contents
#[tauri::command]
pub async fn cat(
    state: State<'_, AppState>,
    bucket_id: String,
    path: String,
) -> Result<CatResult, String> {
    let service = get_service(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;

    let mount = service
        .peer()
        .mount_for_read(bucket_uuid)
        .await
        .map_err(|e| e.to_string())?;

    // Get node for mime type
    let node_link = mount
        .get(&PathBuf::from(&path))
        .await
        .map_err(|e| e.to_string())?;
    let mime_type = node_link_mime(&node_link);

    let content = mount
        .cat(&PathBuf::from(&path))
        .await
        .map_err(|e| e.to_string())?;

    let size = content.len();

    Ok(CatResult {
        content,
        mime_type,
        size,
    })
}

/// Upload a file
#[tauri::command]
pub async fn add_file(
    state: State<'_, AppState>,
    bucket_id: String,
    path: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let service = get_service(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;

    let mut mount = service
        .peer()
        .mount(bucket_uuid)
        .await
        .map_err(|e| e.to_string())?;

    mount
        .add(&PathBuf::from(&path), Cursor::new(data))
        .await
        .map_err(|e| e.to_string())?;

    service
        .peer()
        .save_mount(&mount, false)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Update (overwrite) a file
#[tauri::command]
pub async fn update_file(
    state: State<'_, AppState>,
    bucket_id: String,
    path: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let service = get_service(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;

    let mut mount = service
        .peer()
        .mount(bucket_uuid)
        .await
        .map_err(|e| e.to_string())?;

    // Remove existing then add new content
    let file_path = PathBuf::from(&path);
    let _ = mount.rm(&file_path).await; // Ignore if not found
    mount
        .add(&file_path, Cursor::new(data))
        .await
        .map_err(|e| e.to_string())?;

    service
        .peer()
        .save_mount(&mount, false)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Rename a file or directory
#[tauri::command]
pub async fn rename_path(
    state: State<'_, AppState>,
    bucket_id: String,
    old_path: String,
    new_path: String,
) -> Result<(), String> {
    let service = get_service(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;

    let mut mount = service
        .peer()
        .mount(bucket_uuid)
        .await
        .map_err(|e| e.to_string())?;

    mount
        .mv(&PathBuf::from(&old_path), &PathBuf::from(&new_path))
        .await
        .map_err(|e| e.to_string())?;

    service
        .peer()
        .save_mount(&mount, false)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Move a file or directory
#[tauri::command]
pub async fn move_path(
    state: State<'_, AppState>,
    bucket_id: String,
    source_path: String,
    dest_path: String,
) -> Result<(), String> {
    let service = get_service(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;

    let mut mount = service
        .peer()
        .mount(bucket_uuid)
        .await
        .map_err(|e| e.to_string())?;

    mount
        .mv(&PathBuf::from(&source_path), &PathBuf::from(&dest_path))
        .await
        .map_err(|e| e.to_string())?;

    service
        .peer()
        .save_mount(&mount, false)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Share a bucket with a peer (still uses HTTP — share needs the API flow with key exchange)
#[tauri::command]
pub async fn share_bucket(
    state: State<'_, AppState>,
    bucket_id: String,
    peer_public_key: String,
    role: String,
) -> Result<(), String> {
    let base_url = get_daemon_url(&state).await?;
    let url = format!("{}/api/v0/bucket/share", base_url);

    let bucket_uuid = parse_bucket_id(&bucket_id)?;

    #[derive(Serialize)]
    struct ShareRequest {
        bucket_id: Uuid,
        peer_public_key: String,
        role: String,
    }

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&ShareRequest {
            bucket_id: bucket_uuid,
            peer_public_key,
            role,
        })
        .send()
        .await
        .map_err(|e| format!("Failed to connect to daemon: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Failed to share bucket ({}): {}", status, body));
    }

    Ok(())
}

/// Check if the current HEAD of a bucket is published
#[tauri::command]
pub async fn is_published(state: State<'_, AppState>, bucket_id: String) -> Result<bool, String> {
    let service = get_service(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;

    let mount = service
        .peer()
        .mount_for_read(bucket_uuid)
        .await
        .map_err(|e| e.to_string())?;

    Ok(mount.is_published().await)
}

/// Publish a bucket
#[tauri::command]
pub async fn publish_bucket(state: State<'_, AppState>, bucket_id: String) -> Result<(), String> {
    let service = get_service(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;

    let mount = service
        .peer()
        .mount(bucket_uuid)
        .await
        .map_err(|e| e.to_string())?;

    service
        .peer()
        .save_mount(&mount, true)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Ping a peer (still uses HTTP — ping needs the sync provider)
#[tauri::command]
pub async fn ping_peer(
    state: State<'_, AppState>,
    bucket_id: String,
    peer_public_key: String,
) -> Result<String, String> {
    let base_url = get_daemon_url(&state).await?;
    let url = format!("{}/api/v0/bucket/ping", base_url);

    let bucket_uuid = parse_bucket_id(&bucket_id)?;

    #[derive(Serialize)]
    struct PingRequest {
        bucket_id: Uuid,
        peer_public_key: String,
    }

    #[derive(Deserialize)]
    struct PingResponse {
        message: String,
    }

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&PingRequest {
            bucket_id: bucket_uuid,
            peer_public_key,
        })
        .send()
        .await
        .map_err(|e| format!("Failed to connect to daemon: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Failed to ping peer ({}): {}", status, body));
    }

    let ping_response: PingResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(ping_response.message)
}

/// Upload native files from disk (avoids large IPC transfers)
#[tauri::command]
pub async fn upload_native_files(
    state: State<'_, AppState>,
    bucket_id: String,
    mount_path: String,
    file_paths: Vec<String>,
) -> Result<(), String> {
    let service = get_service(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;

    let mut mount = service
        .peer()
        .mount(bucket_uuid)
        .await
        .map_err(|e| e.to_string())?;

    for file_path in file_paths {
        let path = Path::new(&file_path);
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();

        let data = tokio::fs::read(&file_path)
            .await
            .map_err(|e| format!("Failed to read file '{}': {}", file_path, e))?;

        let dest_path = if mount_path.ends_with('/') {
            format!("{}{}", mount_path, file_name)
        } else {
            format!("{}/{}", mount_path, file_name)
        };

        mount
            .add(&PathBuf::from(&dest_path), Cursor::new(data))
            .await
            .map_err(|e| format!("Failed to add '{}': {}", file_name, e))?;
    }

    service
        .peer()
        .save_mount(&mount, false)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Create a directory
#[tauri::command]
pub async fn mkdir(
    state: State<'_, AppState>,
    bucket_id: String,
    path: String,
) -> Result<(), String> {
    let service = get_service(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;

    let mut mount = service
        .peer()
        .mount(bucket_uuid)
        .await
        .map_err(|e| e.to_string())?;

    mount
        .mkdir(&PathBuf::from(&path))
        .await
        .map_err(|e| e.to_string())?;

    service
        .peer()
        .save_mount(&mount, false)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Delete a file or directory
#[tauri::command]
pub async fn delete_path(
    state: State<'_, AppState>,
    bucket_id: String,
    path: String,
) -> Result<(), String> {
    let service = get_service(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;

    let mut mount = service
        .peer()
        .mount(bucket_uuid)
        .await
        .map_err(|e| e.to_string())?;

    mount
        .rm(&PathBuf::from(&path))
        .await
        .map_err(|e| e.to_string())?;

    service
        .peer()
        .save_mount(&mount, false)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Get bucket version history
#[tauri::command]
pub async fn get_history(
    state: State<'_, AppState>,
    bucket_id: String,
    page: Option<u32>,
) -> Result<Vec<HistoryEntry>, String> {
    let service = get_service(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;
    let db = service.database();

    let entries = db
        .get_bucket_logs(&bucket_uuid, page.unwrap_or(0), 50)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    Ok(entries
        .into_iter()
        .map(|e| HistoryEntry {
            link_hash: e.current_link.to_string(),
            height: e.height,
            published: e.published,
            created_at: e.created_at,
        })
        .collect())
}

/// List directory contents at a specific version
#[tauri::command]
pub async fn ls_at_version(
    state: State<'_, AppState>,
    bucket_id: String,
    link_hash: String,
    path: String,
) -> Result<Vec<FileEntry>, String> {
    let service = get_service(&state).await?;
    let _bucket_uuid = parse_bucket_id(&bucket_id)?;

    let hash: Hash = link_hash
        .parse()
        .map_err(|e| format!("Invalid link hash: {}", e))?;
    let link = Link::new(common::linked_data::LD_RAW_CODEC, hash);

    let mount = Mount::load(&link, service.peer().secret(), service.peer().blobs())
        .await
        .map_err(|e| e.to_string())?;

    let items = mount
        .ls(&PathBuf::from(&path))
        .await
        .map_err(|e| e.to_string())?;

    Ok(items
        .into_iter()
        .map(|(entry_path, node_link)| {
            let name = entry_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let mime_type = node_link_mime(&node_link);
            let link_hash = node_link.link().to_string();
            let is_dir = node_link.is_dir();

            let full_path = if path == "/" {
                format!("/{}", name)
            } else {
                format!("{}/{}", path.trim_end_matches('/'), name)
            };

            FileEntry {
                path: full_path,
                name,
                is_dir,
                mime_type,
                link_hash,
            }
        })
        .collect())
}

/// Share info for a bucket peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareInfo {
    pub public_key: String,
    pub role: String,
    pub is_self: bool,
}

/// Get all shares for a bucket
#[tauri::command]
pub async fn get_bucket_shares(
    state: State<'_, AppState>,
    bucket_id: String,
) -> Result<Vec<ShareInfo>, String> {
    let service = get_service(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;

    let mount = service
        .peer()
        .mount_for_read(bucket_uuid)
        .await
        .map_err(|e| e.to_string())?;

    let inner = mount.inner().await;
    let shares = inner.manifest().shares();

    let self_key = service.peer().secret().public().to_hex();

    Ok(shares
        .iter()
        .map(|(key_hex, share)| {
            let role = match share.role() {
                common::mount::PrincipalRole::Owner => "Owner",
                common::mount::PrincipalRole::Mirror => "Mirror",
            };
            ShareInfo {
                public_key: key_hex.clone(),
                role: role.to_string(),
                is_self: *key_hex == self_key,
            }
        })
        .collect())
}

/// Read file contents at a specific version
#[tauri::command]
pub async fn cat_at_version(
    state: State<'_, AppState>,
    bucket_id: String,
    link_hash: String,
    path: String,
) -> Result<CatResult, String> {
    let service = get_service(&state).await?;
    let _bucket_uuid = parse_bucket_id(&bucket_id)?;

    let hash: Hash = link_hash
        .parse()
        .map_err(|e| format!("Invalid link hash: {}", e))?;
    let link = Link::new(common::linked_data::LD_RAW_CODEC, hash);

    let mount = Mount::load(&link, service.peer().secret(), service.peer().blobs())
        .await
        .map_err(|e| e.to_string())?;

    // Get node for mime type
    let node_link = mount
        .get(&PathBuf::from(&path))
        .await
        .map_err(|e| e.to_string())?;
    let mime_type = node_link_mime(&node_link);

    let content = mount
        .cat(&PathBuf::from(&path))
        .await
        .map_err(|e| e.to_string())?;

    let size = content.len();

    Ok(CatResult {
        content,
        mime_type,
        size,
    })
}
