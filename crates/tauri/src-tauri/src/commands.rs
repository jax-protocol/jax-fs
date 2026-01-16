//! Tauri IPC commands for bucket operations
//!
//! These commands mirror the REST API endpoints but use Tauri's IPC mechanism
//! for communication between the frontend and backend.

use serde::{Deserialize, Serialize};
use std::io::Cursor;
use tauri::State;
use uuid::Uuid;

use common::mount::NodeLink;
use common::prelude::Mount;

use crate::AppState;

/// Bucket info returned to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketInfo {
    pub id: Uuid,
    pub name: String,
    pub height: u64,
}

/// File/directory info returned to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: Option<u64>,
}

/// List all buckets
#[tauri::command]
pub async fn list_buckets(state: State<'_, AppState>) -> Result<Vec<BucketInfo>, String> {
    let db = state.service.database();
    let db_buckets = db.list_buckets(None, None).await.map_err(|e| e.to_string())?;

    Ok(db_buckets
        .into_iter()
        .map(|b| BucketInfo {
            id: b.id,
            name: b.name,
            height: 0, // TODO: Get actual height
        })
        .collect())
}

/// Create a new bucket
#[tauri::command]
pub async fn create_bucket(name: String, state: State<'_, AppState>) -> Result<BucketInfo, String> {
    let peer = state.service.peer();
    let id = Uuid::new_v4();

    // Create new mount (owner is the peer's secret key)
    let mount = Mount::init(id, name.clone(), peer.secret(), peer.blobs())
        .await
        .map_err(|e| e.to_string())?;

    // Save mount and log
    peer.save_mount(&mount).await.map_err(|e| e.to_string())?;

    Ok(BucketInfo {
        id,
        name,
        height: 0,
    })
}

/// Get bucket details
#[tauri::command]
pub async fn get_bucket(bucket_id: Uuid, state: State<'_, AppState>) -> Result<BucketInfo, String> {
    let peer = state.service.peer();
    let mount = peer.mount(bucket_id).await.map_err(|e| e.to_string())?;

    let inner = mount.inner().await;
    let name = inner.manifest().name().to_string();

    Ok(BucketInfo {
        id: bucket_id,
        name,
        height: 0, // TODO: Get actual height
    })
}

/// List files in a bucket directory
#[tauri::command]
pub async fn list_files(
    bucket_id: Uuid,
    path: String,
    state: State<'_, AppState>,
) -> Result<Vec<FileInfo>, String> {
    let peer = state.service.peer();
    let mount = peer.mount(bucket_id).await.map_err(|e| e.to_string())?;

    let path_buf = std::path::PathBuf::from(&path);
    let items = mount.ls(&path_buf).await.map_err(|e| e.to_string())?;

    let mut files = Vec::new();
    for (name, node_link) in items {
        let is_dir = matches!(node_link, NodeLink::Dir(_, _));
        let name_str = name.to_string_lossy().to_string();
        let full_path = if path == "/" {
            format!("/{}", name_str)
        } else {
            format!("{}/{}", path, name_str)
        };

        files.push(FileInfo {
            name: name_str,
            path: full_path,
            is_dir,
            size: None, // TODO: Get actual size
        });
    }

    Ok(files)
}

/// Get file content
#[tauri::command]
pub async fn get_file(
    bucket_id: Uuid,
    path: String,
    state: State<'_, AppState>,
) -> Result<Vec<u8>, String> {
    let peer = state.service.peer();
    let mount = peer.mount(bucket_id).await.map_err(|e| e.to_string())?;

    let path_buf = std::path::PathBuf::from(&path);
    let data = mount.cat(&path_buf).await.map_err(|e| e.to_string())?;

    Ok(data)
}

/// Add a file to a bucket
#[tauri::command]
pub async fn add_file(
    bucket_id: Uuid,
    path: String,
    content: Vec<u8>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let peer = state.service.peer();
    let mut mount = peer.mount(bucket_id).await.map_err(|e| e.to_string())?;

    let path_buf = std::path::PathBuf::from(&path);
    let reader = Cursor::new(content);
    mount
        .add(&path_buf, reader)
        .await
        .map_err(|e| e.to_string())?;

    // Save mount
    peer.save_mount(&mount).await.map_err(|e| e.to_string())?;

    Ok(())
}

/// Delete a file from a bucket
#[tauri::command]
pub async fn delete_file(
    bucket_id: Uuid,
    path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let peer = state.service.peer();
    let mut mount = peer.mount(bucket_id).await.map_err(|e| e.to_string())?;

    let path_buf = std::path::PathBuf::from(&path);
    mount.rm(&path_buf).await.map_err(|e| e.to_string())?;

    // Save mount
    peer.save_mount(&mount).await.map_err(|e| e.to_string())?;

    Ok(())
}

/// Rename a file in a bucket (implemented as copy + delete)
#[tauri::command]
pub async fn rename_file(
    bucket_id: Uuid,
    old_path: String,
    new_path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let peer = state.service.peer();
    let mut mount = peer.mount(bucket_id).await.map_err(|e| e.to_string())?;

    let old_path_buf = std::path::PathBuf::from(&old_path);
    let new_path_buf = std::path::PathBuf::from(&new_path);

    // Read file content
    let content = mount.cat(&old_path_buf).await.map_err(|e| e.to_string())?;

    // Delete old file
    mount.rm(&old_path_buf).await.map_err(|e| e.to_string())?;

    // Add at new path
    let reader = Cursor::new(content);
    mount
        .add(&new_path_buf, reader)
        .await
        .map_err(|e| e.to_string())?;

    // Save mount
    peer.save_mount(&mount).await.map_err(|e| e.to_string())?;

    Ok(())
}

/// Move a file in a bucket
#[tauri::command]
pub async fn move_file(
    bucket_id: Uuid,
    old_path: String,
    new_path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let peer = state.service.peer();
    let mut mount = peer.mount(bucket_id).await.map_err(|e| e.to_string())?;

    let old_path_buf = std::path::PathBuf::from(&old_path);
    let new_path_buf = std::path::PathBuf::from(&new_path);

    // Use mount's mv method
    mount
        .mv(&old_path_buf, &new_path_buf)
        .await
        .map_err(|e| e.to_string())?;

    // Save mount
    peer.save_mount(&mount).await.map_err(|e| e.to_string())?;

    Ok(())
}

/// Create a directory in a bucket
#[tauri::command]
pub async fn create_directory(
    bucket_id: Uuid,
    path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let peer = state.service.peer();
    let mut mount = peer.mount(bucket_id).await.map_err(|e| e.to_string())?;

    let path_buf = std::path::PathBuf::from(&path);
    mount.mkdir(&path_buf).await.map_err(|e| e.to_string())?;

    // Save mount
    peer.save_mount(&mount).await.map_err(|e| e.to_string())?;

    Ok(())
}
