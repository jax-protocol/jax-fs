use askama::Template;
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use common::mount::{Mount, NodeLink};
use serde::Deserialize;

use super::cache::transform::{TransformError, TransformParams};

/// Query parameters for file requests.
#[derive(Debug, Deserialize)]
pub struct FileQuery {
    /// If true, serve with Content-Disposition: attachment
    #[serde(default)]
    pub download: Option<bool>,
    /// If true, use the HTML viewer UI instead of serving raw file
    #[serde(default)]
    pub viewer: Option<bool>,
    /// Target width in pixels for image transforms.
    #[serde(default)]
    pub w: Option<u32>,
    /// Target height in pixels for image transforms.
    #[serde(default)]
    pub h: Option<u32>,
    /// Output quality 1-100 for JPEG/WebP.
    #[serde(default)]
    pub q: Option<u8>,
}

/// Template for file viewer.
#[derive(Template)]
#[template(path = "pages/gateway/viewer.html")]
pub struct GatewayViewerTemplate {
    pub bucket_id: String,
    pub bucket_id_short: String,
    pub bucket_name: String,
    pub bucket_link: String,
    pub bucket_link_short: String,
    pub file_path: String,
    pub file_name: String,
    pub mime_type: String,
    pub size_formatted: String,
    pub content: String,
    pub back_url: String,
}

/// Data that can be cached by the gateway layer.
pub struct CacheableData {
    pub data: Vec<u8>,
    pub mime_type: String,
}

/// Response from the file handler, with optional cacheable data.
pub struct FileResponse {
    pub response: Response,
    pub cacheable: Option<CacheableData>,
}

/// Cache-Control header for cached/transformed responses.
const CACHE_CONTROL_IMMUTABLE: &str = "public, max-age=31536000, immutable";

/// Serve cached content directly, bypassing all mount traversal and decryption.
pub fn serve_cached(data: Bytes, mime_type: &str, absolute_path: &str) -> Response {
    let filename = std::path::Path::new(absolute_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");

    (
        axum::http::StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, mime_type.to_string()),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!("inline; filename=\"{}\"", filename),
            ),
            (
                axum::http::header::CACHE_CONTROL,
                CACHE_CONTROL_IMMUTABLE.to_string(),
            ),
        ],
        data,
    )
        .into_response()
}

// TODO: Query param behavior is documented in templates/pages/gateway/index.html - keep in sync
pub async fn handler(
    mount: &Mount,
    path_buf: &std::path::Path,
    absolute_path: &str,
    query: &FileQuery,
    meta: &super::BucketMeta<'_>,
    node_link: NodeLink,
) -> FileResponse {
    let file_metadata_data = match &node_link {
        NodeLink::Data(_, _, metadata) => metadata.clone(),
        _ => unreachable!("Already checked is_directory"),
    };

    let mime_type = file_metadata_data
        .mime()
        .map(|m| m.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());

    let filename = path_buf
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();

    let wants_download = query.download.unwrap_or(false);
    let wants_viewer = query.viewer.unwrap_or(false);

    // Validate transform params early
    let transform = TransformParams::from_query(query.w, query.h, query.q);
    if let Some(ref params) = transform {
        if let Err(msg) = params.validate() {
            return FileResponse {
                response: (axum::http::StatusCode::BAD_REQUEST, msg.to_string()).into_response(),
                cacheable: None,
            };
        }
    }

    // Read file data
    let file_data = match mount.cat(path_buf).await {
        Ok(data) => data,
        Err(e) => {
            tracing::error!("Failed to read file: {}", e);
            return FileResponse {
                response: super::error_response("Failed to read file"),
                cacheable: None,
            };
        }
    };

    // Apply image transform if requested and applicable
    let (file_data, mime_type) = if let Some(ref params) = transform {
        if TransformParams::applies_to_mime(&mime_type) {
            match super::cache::transform::transform_image(&file_data, &mime_type, params) {
                Ok(transformed) => (transformed, mime_type),
                Err(TransformError::UnsupportedFormat(_)) => {
                    // Not an image type we can transform, serve as-is
                    (file_data, mime_type)
                }
                Err(e) => {
                    tracing::error!("Image transform failed: {}", e);
                    return FileResponse {
                        response: super::error_response("Image transform failed"),
                        cacheable: None,
                    };
                }
            }
        } else {
            // Non-image: ignore transform params, serve normally
            (file_data, mime_type)
        }
    } else {
        (file_data, mime_type)
    };

    let size_formatted = format_bytes(file_data.len());

    // If download is requested, serve with attachment disposition (not cacheable)
    if wants_download {
        return FileResponse {
            response: (
                axum::http::StatusCode::OK,
                [
                    (axum::http::header::CONTENT_TYPE, mime_type.as_str()),
                    (
                        axum::http::header::CONTENT_DISPOSITION,
                        &format!("attachment; filename=\"{}\"", filename),
                    ),
                ],
                file_data,
            )
                .into_response(),
            cacheable: None,
        };
    }

    // When viewer is NOT explicitly set, act like a web server:
    // - Render HTML/Markdown with URL rewriting
    // - Serve other files raw inline
    if query.viewer.is_none() {
        let is_html = mime_type == "text/html";
        let is_markdown = mime_type == "text/markdown";

        if is_html || is_markdown {
            let (final_content, final_mime_type) = if is_markdown {
                let content_str = String::from_utf8_lossy(&file_data);
                let html = super::markdown_to_html(&content_str);
                let rewritten =
                    super::rewrite_relative_urls(&html, absolute_path, meta.id, meta.host);
                (rewritten.into_bytes(), "text/html; charset=utf-8")
            } else {
                let content_str = String::from_utf8_lossy(&file_data);
                let rewritten =
                    super::rewrite_relative_urls(&content_str, absolute_path, meta.id, meta.host);
                (rewritten.into_bytes(), "text/html; charset=utf-8")
            };

            // HTML/Markdown with rewriting is not cacheable (host-dependent)
            return FileResponse {
                response: (
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
                    .into_response(),
                cacheable: None,
            };
        }

        // Non-HTML/Markdown: serve raw and cache
        let cacheable = Some(CacheableData {
            data: file_data.clone(),
            mime_type: mime_type.clone(),
        });

        return FileResponse {
            response: (
                axum::http::StatusCode::OK,
                [
                    (
                        axum::http::header::CONTENT_TYPE,
                        mime_type.as_str().to_string(),
                    ),
                    (
                        axum::http::header::CONTENT_DISPOSITION,
                        format!("inline; filename=\"{}\"", filename),
                    ),
                    (
                        axum::http::header::CACHE_CONTROL,
                        CACHE_CONTROL_IMMUTABLE.to_string(),
                    ),
                ],
                file_data,
            )
                .into_response(),
            cacheable,
        };
    }

    // viewer=false: serve raw file inline
    if !wants_viewer {
        let cacheable = Some(CacheableData {
            data: file_data.clone(),
            mime_type: mime_type.clone(),
        });

        return FileResponse {
            response: (
                axum::http::StatusCode::OK,
                [
                    (
                        axum::http::header::CONTENT_TYPE,
                        mime_type.as_str().to_string(),
                    ),
                    (
                        axum::http::header::CONTENT_DISPOSITION,
                        format!("inline; filename=\"{}\"", filename),
                    ),
                    (
                        axum::http::header::CACHE_CONTROL,
                        CACHE_CONTROL_IMMUTABLE.to_string(),
                    ),
                ],
                file_data,
            )
                .into_response(),
            cacheable,
        };
    }

    // Render file viewer template (not cacheable)
    let content = if mime_type.starts_with("text/")
        || mime_type == "application/json"
        || mime_type == "application/xml"
        || mime_type == "application/javascript"
    {
        String::from_utf8_lossy(&file_data).to_string()
    } else {
        to_hex_dump(&file_data, 1024)
    };

    let back_url = format!("/gw/{}{}", meta.id, get_parent_path(absolute_path));

    let template = GatewayViewerTemplate {
        bucket_id: meta.id_str.to_string(),
        bucket_id_short: meta.id_short.to_string(),
        bucket_name: meta.name.to_string(),
        bucket_link: meta.link.to_string(),
        bucket_link_short: meta.link_short.to_string(),
        file_path: absolute_path.to_string(),
        file_name: filename,
        mime_type,
        size_formatted,
        content,
        back_url,
    };

    FileResponse {
        response: match template.render() {
            Ok(html) => (
                axum::http::StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
                html,
            )
                .into_response(),
            Err(e) => {
                tracing::error!("Failed to render viewer template: {}", e);
                super::error_response("Failed to render page")
            }
        },
        cacheable: None,
    }
}

fn format_bytes(bytes: usize) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];

    if bytes == 0 {
        return "0 B".to_string();
    }

    let bytes_f64 = bytes as f64;
    let k = 1024_f64;
    let i = (bytes_f64.log(k).floor() as usize).min(UNITS.len() - 1);
    let size = bytes_f64 / k.powi(i as i32);

    format!("{:.2} {}", size, UNITS[i])
}

fn get_parent_path(path: &str) -> String {
    if path == "/" {
        return "/".to_string();
    }

    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(0) => "/".to_string(),
        Some(pos) => trimmed[..pos].to_string(),
        None => "/".to_string(),
    }
}

fn to_hex_dump(data: &[u8], max_bytes: usize) -> String {
    let bytes_to_show = data.len().min(max_bytes);
    let mut result = String::new();

    for (i, chunk) in data[..bytes_to_show].chunks(16).enumerate() {
        result.push_str(&format!("{:08x}  ", i * 16));

        for (j, byte) in chunk.iter().enumerate() {
            result.push_str(&format!("{:02x} ", byte));
            if j == 7 {
                result.push(' ');
            }
        }

        for j in chunk.len()..16 {
            result.push_str("   ");
            if j == 7 {
                result.push(' ');
            }
        }

        result.push(' ');

        for byte in chunk {
            if *byte >= 32 && *byte < 127 {
                result.push(*byte as char);
            } else {
                result.push('.');
            }
        }

        result.push('\n');
    }

    if data.len() > max_bytes {
        result.push_str(&format!("\n... ({} more bytes)\n", data.len() - max_bytes));
    }

    result
}
