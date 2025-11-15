use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::{Path, Query, State};
use axum::Extension;
use serde::Deserialize;
use tracing::instrument;
use uuid::Uuid;

use common::linked_data::BlockEncoded;

use crate::daemon::http_server::Config;
use crate::ServiceState;

#[derive(Debug, Clone)]
pub struct FileContent {
    pub data: Vec<u8>,
    pub mime_type: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PathSegment {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ManifestShare {
    pub public_key: String,
    pub role: String,
}

#[derive(Template)]
#[template(path = "pages/buckets/viewer.html")]
pub struct FileViewerTemplate {
    pub bucket_id: String,
    #[allow(dead_code)]
    pub bucket_id_short: String,
    pub bucket_name: String,
    pub bucket_link: String,
    pub bucket_link_short: String,
    pub bucket_data_formatted: String,
    pub manifest_height: u64,
    pub manifest_version: String,
    pub manifest_entry_link: String,
    pub manifest_pins_link: String,
    pub manifest_previous_link: Option<String>,
    pub manifest_shares: Vec<ManifestShare>,
    pub file_path: String,
    pub file_name: String,
    pub path_segments: Vec<PathSegment>,
    pub file_size: usize,
    pub mime_type: String,
    pub is_text: bool,
    pub is_markdown: bool,
    pub is_editable: bool,
    pub content: String,
    pub back_url: String,
    pub viewing_history: bool,
    pub at_hash: Option<String>,
    pub return_url: String,
    pub api_url: String,
    pub read_only: bool,
    pub current_path: String,
}

#[derive(Debug, Deserialize)]
pub struct ViewerQuery {
    pub path: String,
    #[serde(default)]
    pub at: Option<String>,
}

#[instrument(skip(state, config))]
pub async fn handler(
    State(state): State<ServiceState>,
    Extension(config): Extension<Config>,
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

    // Load mount - either from specific link or current state
    let (mount, viewing_link) = if let Some(hash_str) = &query.at {
        // Parse the hash string and create a Link with blake3 codec
        match hash_str.parse::<common::linked_data::Hash>() {
            Ok(hash) => {
                let link = common::linked_data::Link::new(common::linked_data::LD_RAW_CODEC, hash);
                match common::mount::Mount::load(&link, state.peer().secret(), state.peer().blobs())
                    .await
                {
                    Ok(mount) => (mount, link),
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
            Ok(mount) => (mount, bucket.link.clone()),
            Err(e) => {
                tracing::error!("Failed to load mount: {}", e);
                return error_response("Failed to load bucket");
            }
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
    let back_url = build_back_url(&file_path, &bucket_id, query.at.as_ref());

    // Check if file is markdown based on MIME type
    let is_markdown = file_content.mime_type.starts_with("text/markdown");

    // Check if file is editable based on MIME type (text/markdown and text/plain)
    let is_editable = file_content.mime_type.starts_with("text/markdown")
        || file_content.mime_type.starts_with("text/plain");

    let api_url = config
        .api_url
        .clone()
        .unwrap_or_else(|| "http://localhost:3000".to_string());

    let viewing_history = query.at.is_some();
    let return_url = format!("/buckets/{}/view?path={}", bucket_id, file_path);

    // Format bucket link for display - use the viewing_link (current or historical)
    let bucket_link = viewing_link.hash().to_string();
    let bucket_link_short = if bucket_link.len() > 16 {
        format!(
            "{}...{}",
            &bucket_link[..8],
            &bucket_link[bucket_link.len() - 8..]
        )
    } else {
        bucket_link.clone()
    };

    // Format bucket ID for display
    let bucket_id_str = bucket_id.to_string();
    let bucket_id_short = if bucket_id_str.len() > 16 {
        format!(
            "{}...{}",
            &bucket_id_str[..8],
            &bucket_id_str[bucket_id_str.len() - 8..]
        )
    } else {
        bucket_id_str.clone()
    };

    // Load the full bucket data from blobs to format it
    let blobs = state.node().blobs();
    let (
        bucket_data_formatted,
        manifest_height,
        manifest_version,
        manifest_entry_link,
        manifest_pins_link,
        manifest_previous_link,
        manifest_shares,
    ) = match blobs.get(&viewing_link.hash()).await {
        Ok(data) => match common::mount::Manifest::decode(&data) {
            Ok(bucket_data) => {
                // Format bucket data as pretty JSON
                let formatted = serde_json::to_string_pretty(&bucket_data)
                    .unwrap_or_else(|_| format!("{:#?}", bucket_data));

                // Extract manifest fields
                let height = bucket_data.height();
                let version = format!("{:?}", bucket_data.version());
                let entry_link = bucket_data.entry().hash().to_string();
                let pins_link = bucket_data.pins().hash().to_string();
                let previous = bucket_data
                    .previous()
                    .as_ref()
                    .map(|l| l.hash().to_string());
                let shares: Vec<ManifestShare> = bucket_data
                    .shares()
                    .iter()
                    .map(|(pub_key, share)| ManifestShare {
                        public_key: pub_key.clone(),
                        role: format!("{:?}", share.principal().role),
                    })
                    .collect();

                (
                    formatted, height, version, entry_link, pins_link, previous, shares,
                )
            }
            Err(e) => {
                tracing::warn!("Failed to decode bucket data: {}", e);
                (
                    format!("Error decoding bucket data: {}", e),
                    0,
                    String::new(),
                    String::new(),
                    String::new(),
                    None,
                    Vec::new(),
                )
            }
        },
        Err(e) => {
            tracing::warn!("Failed to load bucket data from blobs: {}", e);
            (
                format!("Error loading bucket data: {}", e),
                0,
                String::new(),
                String::new(),
                String::new(),
                None,
                Vec::new(),
            )
        }
    };

    // Get parent directory for current_path
    let current_path = std::path::Path::new(&file_path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("/")
        .to_string();

    let read_only = config.read_only || viewing_history;

    let template = FileViewerTemplate {
        bucket_id: bucket_id.to_string(),
        bucket_id_short,
        bucket_name: bucket.name,
        bucket_link,
        bucket_link_short,
        bucket_data_formatted,
        manifest_height,
        manifest_version,
        manifest_entry_link,
        manifest_pins_link,
        manifest_previous_link,
        manifest_shares,
        file_path,
        file_name,
        path_segments,
        file_size: file_content.data.len(),
        mime_type: file_content.mime_type,
        is_text,
        is_markdown,
        is_editable,
        content,
        back_url,
        viewing_history,
        at_hash: query.at,
        return_url,
        api_url,
        read_only,
        current_path,
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

fn build_back_url(file_path: &str, bucket_id: &Uuid, at_hash: Option<&String>) -> String {
    let parent = std::path::Path::new(file_path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("/");

    if parent == "/" {
        if let Some(hash) = at_hash {
            format!("/buckets/{}?at={}", bucket_id, hash)
        } else {
            format!("/buckets/{}", bucket_id)
        }
    } else if let Some(hash) = at_hash {
        format!("/buckets/{}?path={}&at={}", bucket_id, parent, hash)
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
