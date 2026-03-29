use askama::Template;
use axum::response::{IntoResponse, Response};
use common::mount::{Mount, NodeLink};
use serde::{Deserialize, Serialize};

/// Query parameters for directory listing requests.
#[derive(Debug, Deserialize)]
pub struct DirectoryQuery {
    /// If true, recursively list all files under the path
    #[serde(default)]
    pub deep: Option<bool>,
    /// If true, use the HTML explorer UI instead of raw JSON
    #[serde(default)]
    pub viewer: Option<bool>,
    /// Version hash for pinned browsing.
    #[serde(default)]
    pub at: Option<String>,
}

/// Path segment for breadcrumb navigation.
#[derive(Debug, Clone)]
pub struct PathSegment {
    pub name: String,
    pub path: String,
}

/// File display info for HTML directory listings.
#[derive(Debug, Clone)]
pub struct FileDisplayInfo {
    pub name: String,
    pub path: String,
    pub mime_type: String,
    pub is_dir: bool,
}

/// JSON response for directory listing.
#[derive(Debug, Serialize)]
pub struct DirectoryListing {
    pub path: String,
    pub entries: Vec<DirectoryEntry>,
}

/// Single entry in a JSON directory listing.
#[derive(Debug, Serialize)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub mime_type: String,
}

/// Template for directory explorer.
#[derive(Template)]
#[template(path = "pages/gateway/explorer.html")]
pub struct GatewayExplorerTemplate {
    pub bucket_id: String,
    pub bucket_id_short: String,
    pub bucket_name: String,
    pub bucket_link: String,
    pub bucket_link_short: String,
    pub path_segments: Vec<PathSegment>,
    pub items: Vec<FileDisplayInfo>,
}

// TODO: Query param behavior is documented in templates/pages/gateway/index.html - keep in sync
pub async fn handler(
    mount: &Mount,
    path_buf: &std::path::Path,
    absolute_path: &str,
    query: &DirectoryQuery,
    meta: &super::BucketMeta<'_>,
) -> Response {
    let wants_viewer = query.viewer.unwrap_or(false);

    // When viewer is NOT explicitly set, act like a web server and auto-serve index files.
    // When viewer=true, the user wants to see the directory listing, not the index file.
    if query.viewer.is_none() {
        if let Some((index_path, index_mime_type)) = find_index_file(mount, path_buf).await {
            let file_data = match mount.cat(&index_path).await {
                Ok(data) => data,
                Err(e) => {
                    tracing::error!("Failed to read index file: {}", e);
                    return super::error_response("Failed to read index file");
                }
            };

            let index_path_str = index_path.to_str().unwrap_or(absolute_path);

            let at_hash = query.at.as_deref();
            let (final_content, final_mime_type) = if index_mime_type == "text/markdown" {
                let content_str = String::from_utf8_lossy(&file_data);
                let html = super::rewrite::markdown_to_html(&content_str);
                let rewritten = super::rewrite::rewrite_relative_urls(
                    &html,
                    index_path_str,
                    meta.id,
                    meta.host,
                    at_hash,
                );
                (rewritten.into_bytes(), "text/html; charset=utf-8")
            } else if index_mime_type == "text/html" {
                let content_str = String::from_utf8_lossy(&file_data);
                let rewritten = super::rewrite::rewrite_relative_urls(
                    &content_str,
                    index_path_str,
                    meta.id,
                    meta.host,
                    at_hash,
                );
                (rewritten.into_bytes(), "text/html; charset=utf-8")
            } else {
                (file_data, "text/plain; charset=utf-8")
            };

            return (
                axum::http::StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, final_mime_type)],
                final_content,
            )
                .into_response();
        }
    }

    // List directory contents (deep or shallow based on query param)
    let wants_deep = query.deep.unwrap_or(false);
    let items_map = if wants_deep {
        match mount.ls_deep(path_buf).await {
            Ok(items) => items,
            Err(e) => {
                tracing::error!("Failed to deep list directory: {}", e);
                return super::error_response("Failed to list directory");
            }
        }
    } else {
        match mount.ls(path_buf).await {
            Ok(items) => items,
            Err(e) => {
                tracing::error!("Failed to list directory: {}", e);
                return super::error_response("Failed to list directory");
            }
        }
    };

    // Default: return JSON directory listing
    if !wants_viewer {
        let entries: Vec<DirectoryEntry> = items_map
            .into_iter()
            .map(|(path, node_link)| {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                let mime_type = match &node_link {
                    NodeLink::Dir(_, _) => "inode/directory".to_string(),
                    NodeLink::Data(_, _, data) => data
                        .mime()
                        .map(|m| m.to_string())
                        .unwrap_or_else(|| "application/octet-stream".to_string()),
                };

                DirectoryEntry {
                    name,
                    path: format!("/{}", path.display()),
                    mime_type,
                }
            })
            .collect();

        let listing = DirectoryListing {
            path: absolute_path.to_string(),
            entries,
        };

        return (
            axum::http::StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            serde_json::to_string_pretty(&listing).unwrap(),
        )
            .into_response();
    }

    // Viewer mode: render HTML explorer
    let items: Vec<FileDisplayInfo> = items_map
        .into_iter()
        .map(|(path, node_link)| {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let (mime_type, is_dir) = match &node_link {
                NodeLink::Dir(_, _) => ("inode/directory".to_string(), true),
                NodeLink::Data(_, _, data) => (
                    data.mime()
                        .map(|m| m.to_string())
                        .unwrap_or_else(|| "application/octet-stream".to_string()),
                    false,
                ),
            };

            FileDisplayInfo {
                name,
                path: format!("/{}", path.display()),
                mime_type,
                is_dir,
            }
        })
        .collect();

    let template = GatewayExplorerTemplate {
        bucket_id: meta.id_str.to_string(),
        bucket_id_short: meta.id_short.to_string(),
        bucket_name: meta.name.to_string(),
        bucket_link: meta.link.to_string(),
        bucket_link_short: meta.link_short.to_string(),
        path_segments: build_path_segments(absolute_path),
        items,
    };

    match template.render() {
        Ok(html) => (
            axum::http::StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
            html,
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to render explorer template: {}", e);
            super::error_response("Failed to render page")
        }
    }
}

/// Attempts to find an index file in a directory.
async fn find_index_file(
    mount: &Mount,
    dir_path: &std::path::Path,
) -> Option<(std::path::PathBuf, String)> {
    let candidates = [
        ("index.html", "text/html"),
        ("index.htm", "text/html"),
        ("index.md", "text/markdown"),
        ("index.txt", "text/plain"),
    ];

    for (filename, mime_type) in &candidates {
        let index_path = dir_path.join(filename);
        if mount.get(&index_path).await.is_ok() {
            return Some((index_path, mime_type.to_string()));
        }
    }

    None
}

fn build_path_segments(path: &str) -> Vec<PathSegment> {
    if path == "/" {
        return vec![];
    }

    let mut segments = Vec::new();
    let mut current_path = String::new();

    for part in path.trim_start_matches('/').split('/') {
        if !part.is_empty() {
            current_path = format!("{}/{}", current_path, part);
            segments.push(PathSegment {
                name: part.to_string(),
                path: current_path.clone(),
            });
        }
    }

    segments
}
