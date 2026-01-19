use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use uuid::Uuid;

use common::mount::NodeLink;

use crate::ServiceState;

#[derive(Debug, Deserialize)]
pub struct GatewayQuery {
    #[serde(default)]
    pub at: Option<String>,
}

/// Handler for bucket root requests (no file path)
pub async fn root_handler(
    state: State<ServiceState>,
    Path(bucket_id): Path<Uuid>,
    query: Query<GatewayQuery>,
    headers: axum::http::HeaderMap,
) -> Response {
    // Delegate to main handler with "/" as the path
    handler(state, Path((bucket_id, "/".to_string())), query, headers).await
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

// Lazy static regex patterns for URL rewriting
static HTML_ATTR_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?P<attr>(?:href|src|action|data|srcset))=["'](?P<url>\.{0,2}/[^"']+)["']"#)
        .unwrap()
});

static MARKDOWN_LINK_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\]\((?P<url>\.{0,2}/[^)]+)\)"#).unwrap());

/// Rewrites relative URLs in content to absolute gateway URLs
fn rewrite_relative_urls(
    content: &str,
    current_path: &str,
    bucket_id: &Uuid,
    host: &str,
) -> String {
    let current_dir = if current_path == "/" {
        "".to_string()
    } else {
        std::path::Path::new(current_path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .to_string()
    };

    // Rewrite HTML attributes (href, src, etc.)
    let content = HTML_ATTR_REGEX.replace_all(content, |caps: &regex::Captures| {
        let attr = &caps["attr"];
        let url = &caps["url"];
        let absolute_url = resolve_relative_url(url, &current_dir, bucket_id, host);
        format!(r#"{}="{}""#, attr, absolute_url)
    });

    // Rewrite Markdown links
    let content = MARKDOWN_LINK_REGEX.replace_all(&content, |caps: &regex::Captures| {
        let url = &caps["url"];
        let absolute_url = resolve_relative_url(url, &current_dir, bucket_id, host);
        format!("]({})", absolute_url)
    });

    content.to_string()
}

/// Resolves a relative URL to an absolute gateway URL
fn resolve_relative_url(
    relative_url: &str,
    current_dir: &str,
    bucket_id: &Uuid,
    host: &str,
) -> String {
    let path = if let Some(stripped) = relative_url.strip_prefix("./") {
        // Current directory reference
        format!("{}/{}", current_dir, stripped)
    } else if let Some(stripped) = relative_url.strip_prefix("../") {
        // Parent directory reference
        let parent = std::path::Path::new(current_dir)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("");
        format!("{}/{}", parent, stripped)
    } else if relative_url.starts_with('/') {
        // Already absolute path within bucket
        relative_url.to_string()
    } else {
        // Relative path without ./ prefix
        format!("{}/{}", current_dir, relative_url)
    };

    // Normalize the path and ensure it starts with /
    let normalized = std::path::PathBuf::from(&path).components().fold(
        std::path::PathBuf::new(),
        |mut acc, component| {
            match component {
                std::path::Component::ParentDir => {
                    acc.pop();
                }
                std::path::Component::Normal(part) => {
                    acc.push(part);
                }
                _ => {}
            }
            acc
        },
    );

    let normalized_str = normalized.to_str().unwrap_or("");
    format!(
        "{}/gw/{}/{}",
        host.trim_end_matches('/'),
        bucket_id,
        normalized_str
    )
}

/// Converts markdown content to HTML
fn markdown_to_html(markdown: &str) -> String {
    use pulldown_cmark::{html, Options, Parser};

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    // Wrap in basic HTML structure
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; max-width: 800px; margin: 40px auto; padding: 0 20px; line-height: 1.6; }}
        img {{ max-width: 100%; height: auto; }}
        code {{ background: #f4f4f4; padding: 2px 6px; border-radius: 3px; }}
        pre {{ background: #f4f4f4; padding: 12px; border-radius: 5px; overflow-x: auto; }}
        table {{ border-collapse: collapse; width: 100%; }}
        th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}
        th {{ background-color: #f4f4f4; }}
    </style>
</head>
<body>
{}
</body>
</html>"#,
        html_output
    )
}

/// Attempts to find an index file in a directory
/// Returns (file_path, mime_type) if found
async fn find_index_file(
    mount: &common::mount::Mount,
    dir_path: &std::path::Path,
) -> Option<(std::path::PathBuf, String)> {
    // Priority order: index.html, index.htm, index.md, index.txt
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

pub async fn handler(
    State(state): State<ServiceState>,
    Path((bucket_id, file_path)): Path<(Uuid, String)>,
    Query(query): Query<GatewayQuery>,
    headers: axum::http::HeaderMap,
) -> Response {
    // Extract host from request headers, fallback to localhost
    let host = headers
        .get(axum::http::header::HOST)
        .and_then(|h| h.to_str().ok())
        .map(|h| {
            // Check if the host includes a scheme
            if h.starts_with("http://") || h.starts_with("https://") {
                h.to_string()
            } else {
                // Assume https for production, http for localhost
                if h.contains("localhost") || h.starts_with("127.0.0.1") {
                    format!("http://{}", h)
                } else {
                    format!("https://{}", h)
                }
            }
        })
        .unwrap_or_else(|| "http://localhost".to_string());
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
            // Try to find an index file
            if let Some((index_path, index_mime_type)) = find_index_file(&mount, &path_buf).await {
                // Serve the index file instead of directory listing
                let file_data = match mount.cat(&index_path).await {
                    Ok(data) => data,
                    Err(e) => {
                        tracing::error!("Failed to read index file: {}", e);
                        return error_response("Failed to read index file");
                    }
                };

                // Convert the index_path to string for URL rewriting
                let index_path_str = index_path.to_str().unwrap_or(&absolute_path);

                // Handle different mime types
                let (final_content, final_mime_type) = if index_mime_type == "text/markdown" {
                    // Convert markdown to HTML
                    let content_str = String::from_utf8_lossy(&file_data);
                    let html = markdown_to_html(&content_str);
                    // Apply URL rewriting to the generated HTML
                    let rewritten = rewrite_relative_urls(&html, index_path_str, &bucket_id, &host);
                    (rewritten.into_bytes(), "text/html; charset=utf-8")
                } else if index_mime_type == "text/html" {
                    // Apply URL rewriting to HTML
                    let content_str = String::from_utf8_lossy(&file_data);
                    let rewritten =
                        rewrite_relative_urls(&content_str, index_path_str, &bucket_id, &host);
                    (rewritten.into_bytes(), "text/html; charset=utf-8")
                } else {
                    // Serve text/plain as-is
                    (file_data, "text/plain; charset=utf-8")
                };

                return (
                    axum::http::StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, final_mime_type)],
                    final_content,
                )
                    .into_response();
            }

            // No index file found, return directory listing as JSON
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

            // Apply URL rewriting for text/html and text/markdown files
            let (final_content, final_mime_type) = if mime_type == "text/html" {
                let content_str = String::from_utf8_lossy(&file_data);
                let rewritten =
                    rewrite_relative_urls(&content_str, &absolute_path, &bucket_id, &host);
                (rewritten.into_bytes(), "text/html; charset=utf-8")
            } else if mime_type == "text/markdown" {
                // Convert markdown to HTML and apply URL rewriting
                let content_str = String::from_utf8_lossy(&file_data);
                let html = markdown_to_html(&content_str);
                let rewritten = rewrite_relative_urls(&html, &absolute_path, &bucket_id, &host);
                (rewritten.into_bytes(), "text/html; charset=utf-8")
            } else {
                (file_data, mime_type.as_str())
            };

            (
                axum::http::StatusCode::OK,
                [
                    (axum::http::header::CONTENT_TYPE, final_mime_type),
                    (
                        axum::http::header::CONTENT_DISPOSITION,
                        &format!("inline; filename=\"{}\"", filename),
                    ),
                ],
                final_content,
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
