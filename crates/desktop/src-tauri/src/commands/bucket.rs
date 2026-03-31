//! Bucket IPC commands
//!
//! All commands communicate with the daemon via its HTTP API using the ApiClient,
//! enabling both embedded and sidecar daemon modes.

use serde::{Deserialize, Serialize};
use tauri::State;
use time::OffsetDateTime;
use uuid::Uuid;

use jax_daemon::http_server::api::client::ApiClient;
use jax_daemon::http_server::api::v0::bucket::{
    approve::ApproveRequest,
    cat::CatRequest,
    create::CreateRequest,
    delete::DeleteRequest,
    history::HistoryRequest,
    ignore::IgnoreRequest,
    leave::LeaveRequest,
    list::ListRequest,
    ls::LsRequest,
    mkdir::MkdirRequest,
    mv::MvRequest,
    ping::PingRequest,
    publish::PublishRequest,
    rename::RenameRequest,
    share::{ShareRequest, ShareRole},
    shares::SharesRequest,
    stat::StatRequest,
    unpublish::UnpublishRequest,
    unshare::UnshareRequest,
};

use crate::AppState;

/// Bucket information returned to the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketInfo {
    pub bucket_id: Uuid,
    pub name: String,
    pub link_hash: String,
    pub height: u64,
    pub status: String,
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

/// Share info for a bucket peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareInfo {
    pub public_key: String,
    pub role: String,
    pub is_self: bool,
}

/// Get an ApiClient from the app state.
async fn get_client(state: &State<'_, AppState>) -> Result<ApiClient, String> {
    let inner = state.inner.read().await;
    let inner = inner.as_ref().ok_or("Daemon not connected")?;
    Ok(inner.client.clone())
}

/// Parse a bucket_id string into Uuid
fn parse_bucket_id(bucket_id: &str) -> Result<Uuid, String> {
    bucket_id
        .parse()
        .map_err(|e| format!("Invalid bucket ID: {}", e))
}

/// List all buckets
#[tauri::command]
pub async fn list_buckets(state: State<'_, AppState>) -> Result<Vec<BucketInfo>, String> {
    let mut client = get_client(&state).await?;
    let resp = client
        .call(ListRequest {
            prefix: None,
            limit: None,
            status: None,
        })
        .await
        .map_err(|e| e.to_string())?;

    Ok(resp
        .buckets
        .into_iter()
        .map(|b| BucketInfo {
            bucket_id: b.bucket_id,
            name: b.name,
            link_hash: b.link.to_string(),
            height: 0,
            status: b.status,
            created_at: b.created_at,
        })
        .collect())
}

/// Create a new bucket
#[tauri::command]
pub async fn create_bucket(state: State<'_, AppState>, name: String) -> Result<BucketInfo, String> {
    let mut client = get_client(&state).await?;
    let resp = client
        .call(CreateRequest { name })
        .await
        .map_err(|e| e.to_string())?;

    Ok(BucketInfo {
        bucket_id: resp.bucket_id,
        name: resp.name,
        link_hash: String::new(),
        height: 0,
        status: "active".to_string(),
        created_at: resp.created_at,
    })
}

/// Delete a bucket (deletes at path "/")
#[tauri::command]
pub async fn delete_bucket(state: State<'_, AppState>, bucket_id: String) -> Result<(), String> {
    let mut client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;
    client
        .call(DeleteRequest {
            bucket_id: bucket_uuid,
            path: "/".to_string(),
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Approve a pending bucket
#[tauri::command]
pub async fn approve_bucket(state: State<'_, AppState>, bucket_id: String) -> Result<(), String> {
    let mut client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;
    client
        .call(ApproveRequest {
            bucket_id: bucket_uuid,
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Ignore a pending bucket
#[tauri::command]
pub async fn ignore_bucket(state: State<'_, AppState>, bucket_id: String) -> Result<(), String> {
    let mut client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;
    client
        .call(IgnoreRequest {
            bucket_id: bucket_uuid,
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Leave a bucket
#[tauri::command]
pub async fn leave_bucket(state: State<'_, AppState>, bucket_id: String) -> Result<(), String> {
    let mut client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;
    client
        .call(LeaveRequest {
            bucket_id: bucket_uuid,
        })
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
    let mut client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;
    let resp = client
        .call(LsRequest {
            bucket_id: bucket_uuid,
            path: Some(path),
            deep: None,
            at: None,
        })
        .await
        .map_err(|e| e.to_string())?;

    Ok(resp
        .items
        .into_iter()
        .map(|item| FileEntry {
            path: item.path,
            name: item.name,
            is_dir: item.is_dir,
            mime_type: item.mime_type,
            link_hash: item.link.to_string(),
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
    let mut client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;
    let resp = client
        .call(CatRequest {
            bucket_id: bucket_uuid,
            path,
            at: None,
            download: None,
        })
        .await
        .map_err(|e| e.to_string())?;

    use base64::Engine;
    let content = base64::engine::general_purpose::STANDARD
        .decode(&resp.content)
        .map_err(|e| format!("Failed to decode content: {}", e))?;

    Ok(CatResult {
        content,
        mime_type: resp.mime_type,
        size: resp.size,
    })
}

/// Upload a file via multipart
#[tauri::command]
pub async fn add_file(
    state: State<'_, AppState>,
    bucket_id: String,
    path: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;

    let file_name = std::path::Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();

    let form = reqwest::multipart::Form::new()
        .text("bucket_id", bucket_uuid.to_string())
        .text("mount_path", path)
        .part(
            "file",
            reqwest::multipart::Part::bytes(data).file_name(file_name),
        );

    let url = client.base_url().join("/api/v0/bucket/add").unwrap();
    let response = client
        .http_client()
        .post(url)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to daemon: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Failed to add file ({}): {}", status, body));
    }

    Ok(())
}

/// Update (overwrite) a file via multipart
#[tauri::command]
pub async fn update_file(
    state: State<'_, AppState>,
    bucket_id: String,
    path: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;

    let file_name = std::path::Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();

    let form = reqwest::multipart::Form::new()
        .text("bucket_id", bucket_uuid.to_string())
        .text("mount_path", path)
        .part(
            "file",
            reqwest::multipart::Part::bytes(data).file_name(file_name),
        );

    let url = client.base_url().join("/api/v0/bucket/update").unwrap();
    let response = client
        .http_client()
        .post(url)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to daemon: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Failed to update file ({}): {}", status, body));
    }

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
    let mut client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;
    client
        .call(RenameRequest {
            bucket_id: bucket_uuid,
            old_path,
            new_path,
        })
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
    let mut client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;
    client
        .call(MvRequest {
            bucket_id: bucket_uuid,
            source_path,
            dest_path,
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Share a bucket with a peer
#[tauri::command]
pub async fn share_bucket(
    state: State<'_, AppState>,
    bucket_id: String,
    peer_public_key: String,
    role: String,
) -> Result<(), String> {
    let mut client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;
    let share_role = match role.to_lowercase().as_str() {
        "mirror" => ShareRole::Mirror,
        _ => ShareRole::Owner,
    };
    client
        .call(ShareRequest {
            bucket_id: bucket_uuid,
            peer_public_key,
            role: share_role,
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Remove a share from a bucket
#[tauri::command]
pub async fn remove_share(
    state: State<'_, AppState>,
    bucket_id: String,
    peer_public_key: String,
) -> Result<(), String> {
    let mut client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;
    client
        .call(UnshareRequest {
            bucket_id: bucket_uuid,
            peer_public_key,
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Check if the current HEAD of a bucket is published (via stat endpoint)
#[tauri::command]
pub async fn is_published(state: State<'_, AppState>, bucket_id: String) -> Result<bool, String> {
    let mut client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;
    let resp = client
        .call(StatRequest {
            bucket_id: bucket_uuid,
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.published)
}

/// Publish a bucket
#[tauri::command]
pub async fn publish_bucket(state: State<'_, AppState>, bucket_id: String) -> Result<(), String> {
    let mut client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;
    client
        .call(PublishRequest {
            bucket_id: bucket_uuid,
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Unpublish a bucket
#[tauri::command]
pub async fn unpublish_bucket(state: State<'_, AppState>, bucket_id: String) -> Result<(), String> {
    let mut client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;
    client
        .call(UnpublishRequest {
            bucket_id: bucket_uuid,
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Ping a peer
#[tauri::command]
pub async fn ping_peer(
    state: State<'_, AppState>,
    bucket_id: String,
    peer_public_key: String,
) -> Result<String, String> {
    let mut client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;
    let resp = client
        .call(PingRequest {
            bucket_id: bucket_uuid,
            peer_public_key,
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.message)
}

/// Upload native files from disk via multipart (avoids large IPC transfers)
#[tauri::command]
pub async fn upload_native_files(
    state: State<'_, AppState>,
    bucket_id: String,
    mount_path: String,
    file_paths: Vec<String>,
) -> Result<(), String> {
    let client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;

    let url = client.base_url().join("/api/v0/bucket/add").unwrap();

    for file_path in file_paths {
        let path = std::path::Path::new(&file_path);
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

        let form = reqwest::multipart::Form::new()
            .text("bucket_id", bucket_uuid.to_string())
            .text("mount_path", dest_path)
            .part(
                "file",
                reqwest::multipart::Part::bytes(data).file_name(file_name),
            );

        let response = client
            .http_client()
            .post(url.clone())
            .multipart(form)
            .send()
            .await
            .map_err(|e| format!("Failed to connect to daemon: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Failed to upload file ({}): {}", status, body));
        }
    }

    Ok(())
}

/// Create a directory
#[tauri::command]
pub async fn mkdir(
    state: State<'_, AppState>,
    bucket_id: String,
    path: String,
) -> Result<(), String> {
    let mut client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;
    client
        .call(MkdirRequest {
            bucket_id: bucket_uuid,
            path,
        })
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
    let mut client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;
    client
        .call(DeleteRequest {
            bucket_id: bucket_uuid,
            path,
        })
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
    let mut client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;
    let resp = client
        .call(HistoryRequest {
            bucket_id: bucket_uuid,
            page,
            page_size: Some(50),
        })
        .await
        .map_err(|e| e.to_string())?;

    Ok(resp
        .entries
        .into_iter()
        .map(|e| HistoryEntry {
            link_hash: e.link_hash,
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
    let mut client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;
    let resp = client
        .call(LsRequest {
            bucket_id: bucket_uuid,
            path: Some(path),
            deep: None,
            at: Some(link_hash),
        })
        .await
        .map_err(|e| e.to_string())?;

    Ok(resp
        .items
        .into_iter()
        .map(|item| FileEntry {
            path: item.path,
            name: item.name,
            is_dir: item.is_dir,
            mime_type: item.mime_type,
            link_hash: item.link.to_string(),
        })
        .collect())
}

/// Get all shares for a bucket
#[tauri::command]
pub async fn get_bucket_shares(
    state: State<'_, AppState>,
    bucket_id: String,
) -> Result<Vec<ShareInfo>, String> {
    let mut client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;
    let resp = client
        .call(SharesRequest {
            bucket_id: bucket_uuid,
        })
        .await
        .map_err(|e| e.to_string())?;

    Ok(resp
        .shares
        .into_iter()
        .map(|s| ShareInfo {
            public_key: s.public_key,
            role: s.role,
            is_self: s.is_self,
        })
        .collect())
}

/// Export a file to the native filesystem
#[tauri::command]
pub async fn export_file(
    state: State<'_, AppState>,
    bucket_id: String,
    path: String,
    dest_path: String,
) -> Result<(), String> {
    let result = cat(state, bucket_id, path).await?;
    tokio::fs::write(&dest_path, &result.content)
        .await
        .map_err(|e| format!("Failed to write file '{}': {}", dest_path, e))?;
    Ok(())
}

/// Read file contents at a specific version
#[tauri::command]
pub async fn cat_at_version(
    state: State<'_, AppState>,
    bucket_id: String,
    link_hash: String,
    path: String,
) -> Result<CatResult, String> {
    let mut client = get_client(&state).await?;
    let bucket_uuid = parse_bucket_id(&bucket_id)?;
    let resp = client
        .call(CatRequest {
            bucket_id: bucket_uuid,
            path,
            at: Some(link_hash),
            download: None,
        })
        .await
        .map_err(|e| e.to_string())?;

    use base64::Engine;
    let content = base64::engine::general_purpose::STANDARD
        .decode(&resp.content)
        .map_err(|e| format!("Failed to decode content: {}", e))?;

    Ok(CatResult {
        content,
        mime_type: resp.mime_type,
        size: resp.size,
    })
}
