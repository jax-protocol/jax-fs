use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use regex::Regex;
use serde::Deserialize;
use std::sync::LazyLock;
use uuid::Uuid;

use common::mount::NodeLink;

use crate::ServiceState;

pub mod cache;
pub mod directory;
pub mod file;
pub mod index;
pub mod version;

/// Bucket metadata passed to sub-handlers.
pub struct BucketMeta<'a> {
    pub id: &'a Uuid,
    pub id_str: &'a str,
    pub id_short: &'a str,
    pub name: &'a str,
    pub link: &'a str,
    pub link_short: &'a str,
    pub host: &'a str,
}

// Lazy static regex patterns for URL rewriting
static HTML_ATTR_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?P<attr>(?:href|src|action|data|srcset))=["'](?P<url>\.{0,2}/[^"']+)["']"#)
        .unwrap()
});

static MARKDOWN_LINK_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\]\((?P<url>\.{0,2}/[^)]+)\)"#).unwrap());

/// Unified query parameters deserialized from the URL.
/// Individual handler modules define their own typed queries; this captures the
/// superset so the router can forward to the correct handler.
#[derive(Debug, Deserialize)]
pub struct GatewayQuery {
    #[serde(default)]
    pub at: Option<String>,
    #[serde(default)]
    pub download: Option<bool>,
    #[serde(default)]
    pub deep: Option<bool>,
    /// If true, use the HTML viewer UI instead of raw JSON/binary responses.
    #[serde(default)]
    pub viewer: Option<bool>,
    /// Target width in pixels for image transforms (maintains aspect ratio if h omitted).
    #[serde(default)]
    pub w: Option<u32>,
    /// Target height in pixels for image transforms (optional).
    #[serde(default)]
    pub h: Option<u32>,
    /// Output quality 1-100 for JPEG/WebP (default: 80).
    #[serde(default)]
    pub q: Option<u8>,
}

/// Handler for bucket root requests (no file path).
pub async fn root_handler(
    state: State<ServiceState>,
    Path(bucket_id): Path<Uuid>,
    query: Query<GatewayQuery>,
    headers: axum::http::HeaderMap,
) -> Response {
    handler(state, Path((bucket_id, "/".to_string())), query, headers).await
}

pub async fn handler(
    State(state): State<ServiceState>,
    Path((bucket_id, file_path)): Path<(Uuid, String)>,
    Query(query): Query<GatewayQuery>,
    headers: axum::http::HeaderMap,
) -> Response {
    // Extract host from request headers
    let host = headers
        .get(axum::http::header::HOST)
        .and_then(|h| h.to_str().ok())
        .map(|h| {
            if h.starts_with("http://") || h.starts_with("https://") {
                h.to_string()
            } else if h.contains("localhost") || h.starts_with("127.0.0.1") {
                format!("http://{}", h)
            } else {
                format!("https://{}", h)
            }
        })
        .unwrap_or_else(|| "http://localhost".to_string());

    // Ensure path is absolute
    let absolute_path = if file_path.starts_with('/') {
        file_path
    } else {
        format!("/{}", file_path)
    };

    // Load mount - either from specific link or latest published version
    let mount = if let Some(hash_str) = &query.at {
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
        use common::bucket_log::BucketLogProvider;
        match state.peer().logs().latest_published(bucket_id).await {
            Ok(Some((published_link, _height))) => {
                match common::mount::Mount::load(
                    &published_link,
                    state.peer().secret(),
                    state.peer().blobs(),
                )
                .await
                {
                    Ok(mount) => mount,
                    Err(_) => {
                        return syncing_response();
                    }
                }
            }
            _ => {
                return syncing_response();
            }
        }
    };

    let path_buf = std::path::PathBuf::from(&absolute_path);

    let is_root = absolute_path == "/";

    let node_link = if is_root {
        None
    } else {
        match mount.get(&path_buf).await {
            Ok(node) => Some(node),
            Err(e) => {
                tracing::error!("Failed to get path {}: {}", absolute_path, e);
                return not_found_response(&format!("Path not found: {}", absolute_path));
            }
        }
    };

    let is_directory = match &node_link {
        None => true,
        Some(NodeLink::Dir(_, _)) => true,
        Some(NodeLink::Data(_, _, _)) => false,
    };

    // Get bucket metadata from mount
    let inner = mount.inner().await;
    let bucket_name = inner.manifest().name().to_string();
    let bucket_id_str = bucket_id.to_string();
    let bucket_id_short = format!(
        "{}...{}",
        &bucket_id_str[..8],
        &bucket_id_str[bucket_id_str.len() - 4..]
    );
    let bucket_link = inner.link().hash().to_string();
    let bucket_link_short = format!(
        "{}...{}",
        &bucket_link[..8],
        &bucket_link[bucket_link.len() - 8..]
    );

    let meta = BucketMeta {
        id: &bucket_id,
        id_str: &bucket_id_str,
        id_short: &bucket_id_short,
        name: &bucket_name,
        link: &bucket_link,
        link_short: &bucket_link_short,
        host: &host,
    };

    if is_directory {
        let dir_query = directory::DirectoryQuery {
            deep: query.deep,
            viewer: query.viewer,
        };
        directory::handler(&mount, &path_buf, &absolute_path, &dir_query, &meta).await
    } else {
        let file_query = file::FileQuery {
            download: query.download,
            viewer: query.viewer,
            w: query.w,
            h: query.h,
            q: query.q,
        };

        // Skip cache for historical version requests (?at=) since the height
        // from the mount may not match the historical version's actual position
        // in the log, which could cause cache key collisions.
        let use_cache = query.at.is_none();
        let gw_cache = if use_cache {
            state.gateway_cache()
        } else {
            None
        };

        let height = inner.height() as i64;
        let transform_params_key =
            cache::transform::TransformParams::from_query(query.w, query.h, query.q)
                .map(|p| p.to_cache_key())
                .unwrap_or_default();

        // Check cache before traversal/decrypt
        if let Some(gw_cache) = gw_cache {
            if let Some((cached_bytes, cached_mime)) = gw_cache
                .get(
                    &bucket_id_str,
                    height,
                    &absolute_path,
                    &transform_params_key,
                )
                .await
            {
                tracing::debug!(path = %absolute_path, "gateway cache hit");
                return file::serve_cached(cached_bytes, &cached_mime, &absolute_path);
            }
        }

        let response_data = file::handler(
            &mount,
            &path_buf,
            &absolute_path,
            &file_query,
            &meta,
            node_link.unwrap(),
        )
        .await;

        // Populate cache on miss (for non-viewer, non-download, non-HTML responses)
        if let (Some(gw_cache), Some(ref cacheable)) = (gw_cache, &response_data.cacheable) {
            gw_cache
                .put(
                    &bucket_id_str,
                    height,
                    &absolute_path,
                    &transform_params_key,
                    &cacheable.data,
                    &cacheable.mime_type,
                )
                .await;
        }

        response_data.response
    }
}

pub(super) fn error_response(message: &str) -> Response {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        format!("Error: {}", message),
    )
        .into_response()
}

pub(crate) fn syncing_response() -> Response {
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

/// Rewrites relative URLs in content to absolute gateway URLs.
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

    let content = HTML_ATTR_REGEX.replace_all(content, |caps: &regex::Captures| {
        let attr = &caps["attr"];
        let url = &caps["url"];
        let absolute_url = resolve_relative_url(url, &current_dir, bucket_id, host);
        format!(r#"{}="{}""#, attr, absolute_url)
    });

    let content = MARKDOWN_LINK_REGEX.replace_all(&content, |caps: &regex::Captures| {
        let url = &caps["url"];
        let absolute_url = resolve_relative_url(url, &current_dir, bucket_id, host);
        format!("]({})", absolute_url)
    });

    content.to_string()
}

fn resolve_relative_url(
    relative_url: &str,
    current_dir: &str,
    bucket_id: &Uuid,
    host: &str,
) -> String {
    let path = if let Some(stripped) = relative_url.strip_prefix("./") {
        format!("{}/{}", current_dir, stripped)
    } else if let Some(stripped) = relative_url.strip_prefix("../") {
        let parent = std::path::Path::new(current_dir)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("");
        format!("{}/{}", parent, stripped)
    } else if relative_url.starts_with('/') {
        relative_url.to_string()
    } else {
        format!("{}/{}", current_dir, relative_url)
    };

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

/// Converts markdown content to HTML.
fn markdown_to_html(markdown: &str) -> String {
    use pulldown_cmark::{html, Options, Parser};

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

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
