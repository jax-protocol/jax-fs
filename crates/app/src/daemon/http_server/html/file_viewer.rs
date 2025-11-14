use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use tracing::instrument;
use uuid::Uuid;

use crate::ServiceState;

#[derive(Debug, Clone)]
pub struct FileContent {
    pub data: Vec<u8>,
    pub mime_type: String,
}

#[derive(Debug, Clone)]
pub struct PathSegment {
    pub name: String,
    pub path: String,
}

#[derive(Template)]
#[template(path = "file_viewer.html")]
pub struct FileViewerTemplate {
    pub bucket_id: String,
    pub bucket_name: String,
    pub file_path: String,
    pub file_name: String,
    pub path_segments: Vec<PathSegment>,
    pub file_size: usize,
    pub mime_type: String,
    pub is_text: bool,
    pub content: String,
    pub back_url: String,
}

#[derive(Debug, Deserialize)]
pub struct ViewerQuery {
    pub path: String,
}

#[instrument(skip(state))]
pub async fn handler(
    State(state): State<ServiceState>,
    Path(bucket_id): Path<Uuid>,
    Query(query): Query<ViewerQuery>,
) -> askama_axum::Response {
    let file_path = query.path;

    // Get bucket info from database
    let bucket = match state.database().get_bucket_info(&bucket_id).await {
        Ok(Some(bucket)) => bucket,
        Ok(None) => return error_response("Bucket not found"),
        Err(e) => return error_response(&format!("{}", e)),
    };

    // Load mount and get file content
    let mount = match state.peer().mount(bucket_id).await {
        Ok(mount) => mount,
        Err(e) => {
            tracing::error!("Failed to load mount: {}", e);
            return error_response("Failed to load bucket");
        }
    };

    let path_buf = std::path::PathBuf::from(&file_path);

    let file_data = match mount.cat(&path_buf).await {
        Ok(data) => data,
        Err(e) => {
            tracing::error!("Failed to get file content: {}", e);
            return error_response("Failed to load file content");
        }
    };

    let node_link = match mount.get(&path_buf).await {
        Ok(node) => node,
        Err(e) => {
            tracing::error!("Failed to get file metadata: {}", e);
            return error_response("Failed to load file metadata");
        }
    };

    let file_mime_type = node_link
        .data()
        .and_then(|data| data.mime())
        .map(|mime| mime.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());

    let file_content = FileContent {
        data: file_data,
        mime_type: file_mime_type,
    };

    // Extract file name
    let file_name = std::path::Path::new(&file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&file_path)
        .to_string();

    // Determine how to display content based on MIME type
    let (is_text, content) = if file_content.mime_type.starts_with("image/")
        || file_content.mime_type.starts_with("video/")
        || file_content.mime_type.starts_with("audio/")
    {
        // Encode as base64 for embedded display
        (
            false,
            base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &file_content.data,
            ),
        )
    } else {
        // Try to decode as UTF-8 text
        match String::from_utf8(file_content.data.clone()) {
            Ok(text) => (true, text),
            Err(_) => {
                // Binary content - show hex dump
                let hex = file_content
                    .data
                    .chunks(16)
                    .enumerate()
                    .map(|(i, chunk)| {
                        let hex_part: String = chunk
                            .iter()
                            .map(|b| format!("{:02x}", b))
                            .collect::<Vec<_>>()
                            .join(" ");
                        let ascii_part: String = chunk
                            .iter()
                            .map(|&b| {
                                if b.is_ascii_graphic() || b == b' ' {
                                    b as char
                                } else {
                                    '.'
                                }
                            })
                            .collect();
                        format!("{:08x}  {:47}  |{}|", i * 16, hex_part, ascii_part)
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                (false, hex)
            }
        }
    };

    // Build path segments for breadcrumb (excluding the file name)
    let path_segments = build_path_segments(&file_path);

    // Build back URL (parent directory)
    let back_url = build_back_url(&file_path, &bucket_id);

    let template = FileViewerTemplate {
        bucket_id: bucket_id.to_string(),
        bucket_name: bucket.name,
        file_path,
        file_name,
        path_segments,
        file_size: file_content.data.len(),
        mime_type: file_content.mime_type,
        is_text,
        content,
        back_url,
    };

    template.into_response()
}

fn build_path_segments(file_path: &str) -> Vec<PathSegment> {
    // Get the directory path (everything except the file name)
    let parent = std::path::Path::new(file_path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("/");

    if parent == "/" {
        return vec![];
    }

    let parts: Vec<&str> = parent.trim_matches('/').split('/').collect();
    let mut segments = Vec::new();
    let mut accumulated = String::new();

    for part in parts {
        accumulated.push('/');
        accumulated.push_str(part);
        segments.push(PathSegment {
            name: part.to_string(),
            path: accumulated.clone(),
        });
    }

    segments
}

fn build_back_url(file_path: &str, bucket_id: &Uuid) -> String {
    let parent = std::path::Path::new(file_path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("/");

    if parent == "/" {
        format!("/buckets/{}", bucket_id)
    } else {
        format!("/buckets/{}?path={}", bucket_id, parent)
    }
}

fn error_response(message: &str) -> askama_axum::Response {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        format!("Error: {}", message),
    )
        .into_response()
}
