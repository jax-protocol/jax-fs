use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use common::mount::NodeLink;

use crate::ServiceState;

#[derive(Debug, Deserialize)]
pub struct GatewayQuery {
    #[serde(default)]
    pub at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DirectoryListing {
    pub path: String,
    pub entries: Vec<DirectoryEntry>,
}

#[derive(Debug, Serialize)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub mime_type: String,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Path((bucket_id, file_path)): Path<(Uuid, String)>,
    Query(query): Query<GatewayQuery>,
) -> Response {
    // Ensure path is absolute
    let absolute_path = if file_path.starts_with('/') {
        file_path
    } else {
        format!("/{}", file_path)
    };

    // Load mount - either from specific link or current state
    let mount = if let Some(hash_str) = &query.at {
        // Parse the hash string and create a Link
        match hash_str.parse::<common::linked_data::Hash>() {
            Ok(hash) => {
                let link = common::linked_data::Link::new(common::linked_data::LD_RAW_CODEC, hash);
                match common::mount::Mount::load(&link, state.peer().secret(), state.peer().blobs())
                    .await
                {
                    Ok(mount) => mount,
                    Err(e) => {
                        tracing::error!("Failed to load mount from link: {}", e);
                        return error_response("Failed to load historical version");
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to parse hash: {}", e);
                return error_response("Invalid hash format");
            }
        }
    } else {
        match state.peer().mount(bucket_id).await {
            Ok(mount) => mount,
            Err(common::prelude::MountError::LinkNotFound(_)) => {
                return syncing_response();
            }
            Err(e) => {
                tracing::error!("Failed to load mount: {}", e);
                return error_response("Failed to load bucket");
            }
        }
    };

    let path_buf = std::path::PathBuf::from(&absolute_path);

    // Try to get the node to determine if it's a file or directory
    let node_link = match mount.get(&path_buf).await {
        Ok(node) => node,
        Err(e) => {
            tracing::error!("Failed to get path {}: {}", absolute_path, e);
            return not_found_response(&format!("Path not found: {}", absolute_path));
        }
    };

    // Check if it's a directory
    match node_link {
        NodeLink::Dir(_, _) => {
            // List directory contents
            let items_map = match mount.ls(&path_buf).await {
                Ok(items) => items,
                Err(e) => {
                    tracing::error!("Failed to list directory: {}", e);
                    return error_response("Failed to list directory");
                }
            };

            let entries: Vec<DirectoryEntry> = items_map
                .into_iter()
                .map(|(path, node_link)| {
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();
                    let is_dir = matches!(node_link, NodeLink::Dir(_, _));

                    let mime_type = if is_dir {
                        "inode/directory".to_string()
                    } else {
                        match &node_link {
                            NodeLink::Data(_, _, data) => data
                                .mime()
                                .map(|m| m.to_string())
                                .unwrap_or_else(|| "application/octet-stream".to_string()),
                            NodeLink::Dir(_, _) => "inode/directory".to_string(),
                        }
                    };

                    DirectoryEntry {
                        name,
                        path: format!("/{}", path.display()),
                        is_dir,
                        mime_type,
                    }
                })
                .collect();

            let listing = DirectoryListing {
                path: absolute_path,
                entries,
            };

            (
                axum::http::StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                serde_json::to_string_pretty(&listing).unwrap(),
            )
                .into_response()
        }
        NodeLink::Data(_, _, file_metadata) => {
            // Serve file content
            let file_data = match mount.cat(&path_buf).await {
                Ok(data) => data,
                Err(e) => {
                    tracing::error!("Failed to read file: {}", e);
                    return error_response("Failed to read file");
                }
            };

            let mime_type = file_metadata
                .mime()
                .map(|m| m.to_string())
                .unwrap_or_else(|| "application/octet-stream".to_string());

            // Get filename for Content-Disposition
            let filename = path_buf
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file");

            (
                axum::http::StatusCode::OK,
                [
                    (axum::http::header::CONTENT_TYPE, mime_type.as_str()),
                    (
                        axum::http::header::CONTENT_DISPOSITION,
                        &format!("inline; filename=\"{}\"", filename),
                    ),
                ],
                file_data,
            )
                .into_response()
        }
    }
}

fn error_response(message: &str) -> Response {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        format!("Error: {}", message),
    )
        .into_response()
}

fn syncing_response() -> Response {
    (
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        [(axum::http::header::RETRY_AFTER, "5")],
        "Bucket is still syncing. Please try again in a moment.",
    )
        .into_response()
}

fn not_found_response(message: &str) -> Response {
    (
        axum::http::StatusCode::NOT_FOUND,
        format!("Not found: {}", message),
    )
        .into_response()
}
