use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use uuid::Uuid;

use common::mount::NodeLink;

use crate::ServiceState;

pub mod cache;
pub mod directory;
pub mod file;
pub mod index;
pub mod rewrite;
pub mod transform;
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
    gw_cache: cache::GatewayCache,
    Path(bucket_id): Path<Uuid>,
    query: Query<GatewayQuery>,
    headers: axum::http::HeaderMap,
) -> Response {
    handler(
        state,
        gw_cache,
        Path((bucket_id, "/".to_string())),
        query,
        headers,
    )
    .await
}

pub async fn handler(
    State(state): State<ServiceState>,
    gw_cache: cache::GatewayCache,
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
            at: query.at.clone(),
        };
        directory::handler(&mount, &path_buf, &absolute_path, &dir_query, &meta).await
    } else {
        let file_query = file::FileQuery {
            download: query.download,
            viewer: query.viewer,
            w: query.w,
            h: query.h,
            q: query.q,
            at: query.at.clone(),
        };

        let height = inner.height();
        let cache_query_string = transform::TransformParams::from_query(query.w, query.h, query.q)
            .map(|p| p.to_query_string());
        let cache_qs_ref = cache_query_string.as_deref();

        // Skip cache for historical version requests (?at=) — the height
        // may not match the historical version's position in the log.
        let use_cache = query.at.is_none();

        // Check cache before traversal/decrypt
        if use_cache {
            if let Some((cached_bytes, cached_mime)) = gw_cache
                .get(&bucket_id, height, &path_buf, cache_qs_ref)
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
        if use_cache {
            if let Some(ref cacheable) = response_data.cacheable {
                gw_cache
                    .put(
                        &bucket_id,
                        height,
                        &path_buf,
                        cache_qs_ref,
                        &cacheable.link,
                        &cacheable.data,
                        &cacheable.mime_type,
                    )
                    .await;
            }
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
