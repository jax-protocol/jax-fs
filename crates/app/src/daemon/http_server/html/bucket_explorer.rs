use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Extension;
use serde::Deserialize;
use tracing::instrument;
use uuid::Uuid;

use common::linked_data::BlockEncoded;
use common::mount::Manifest;

use crate::daemon::http_server::Config;
use crate::ServiceState;

#[derive(Template)]
#[template(path = "pages/buckets/syncing.html")]
pub struct SyncingTemplate {
    pub bucket_id: String,
    pub bucket_name: String,
}

#[derive(Template)]
#[template(path = "pages/buckets/not_found.html")]
pub struct BucketNotFoundTemplate {
    pub bucket_id: String,
}

#[derive(Template)]
#[template(path = "pages/buckets/index.html")]
pub struct BucketExplorerTemplate {
    pub bucket_id: String,
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
    pub current_path: String,
    pub path_segments: Vec<PathSegment>,
    pub parent_path_url: String,
    pub items: Vec<FileDisplayInfo>,
    pub read_only: bool,
    pub viewing_history: bool,
    pub at_hash: Option<String>,
    pub return_url: String,
    pub api_url: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ManifestShare {
    pub public_key: String,
    pub role: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PathSegment {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct FileDisplayInfo {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub mime_type: String,
    pub file_size: String,
    pub modified_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ExplorerQuery {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub at: Option<String>,
}

#[instrument(skip(state, config))]
pub async fn handler(
    State(state): State<ServiceState>,
    Extension(config): Extension<Config>,
    _headers: HeaderMap,
    Path(bucket_id): Path<Uuid>,
    Query(query): Query<ExplorerQuery>,
) -> askama_axum::Response {
    // If viewing a specific version, always make it read-only
    let viewing_history = query.at.is_some();
    let read_only = config.read_only || viewing_history;

    let current_path = query.path.unwrap_or_else(|| "/".to_string());

    // Get bucket info from bucket_log
    let bucket = match state.database().get_bucket_info(&bucket_id).await {
        Ok(Some(bucket)) => bucket,
        Ok(None) => {
            let template = BucketNotFoundTemplate {
                bucket_id: bucket_id.to_string(),
            };
            return template.into_response();
        }
        Err(e) => return error_response(&format!("{}", e)),
    };

    // Load mount - either from specific link or current state
    let (mount, viewing_link) = if let Some(hash_str) = &query.at {
        // Parse the hash string and create a Link with blake3 codec
        match hash_str.parse::<common::linked_data::Hash>() {
            Ok(hash) => {
                // Create a Link from the hash using the raw codec
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
        // Load current version
        match state.peer().mount(bucket_id).await {
            Ok(mount) => (mount, bucket.link.clone()),
            // TODO (amiller68): this is far too broad, but fine for
            //  now
            Err(_e) => {
                let template = SyncingTemplate {
                    bucket_id: bucket_id.to_string(),
                    bucket_name: bucket.name.clone(),
                };
                return template.into_response();
            }
        }
    };

    let path_buf = std::path::PathBuf::from(&current_path);
    let items_map = match mount.ls(&path_buf).await {
        Ok(items) => items,
        Err(e) => {
            tracing::error!("Failed to list bucket contents: {}", e);
            return error_response("Failed to load bucket contents");
        }
    };

    // Build path segments for breadcrumb
    let path_segments = build_path_segments(&current_path);

    // Build parent path URL
    let parent_path_url = build_parent_path_url(&current_path, &bucket_id, query.at.as_ref());

    // Convert BTreeMap<PathBuf, NodeLink> to display format
    let display_items: Vec<FileDisplayInfo> = items_map
        .into_iter()
        .map(|(path, node_link)| {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let is_dir = matches!(node_link, common::mount::NodeLink::Dir(..));

            let (mime_type, file_size) = if is_dir {
                ("inode/directory".to_string(), "-".to_string())
            } else {
                let mime = node_link
                    .data()
                    .and_then(|data| data.mime())
                    .map(|m| m.to_string())
                    .unwrap_or_else(|| "application/octet-stream".to_string());

                // TODO: Get actual file size from blob when we have efficient access
                // For now, show placeholder
                let size = "-".to_string();

                (mime, size)
            };

            // TODO: Get actual modified date from metadata when available
            // For now, use "Today" as placeholder
            let modified_at = "Today".to_string();

            FileDisplayInfo {
                name,
                path: format!("/{}", path.display()),
                is_dir,
                mime_type,
                file_size,
                modified_at,
            }
        })
        .collect();

    let api_url = config
        .api_url
        .clone()
        .unwrap_or_else(|| "http://localhost:3000".to_string());

    tracing::info!(
        "BUCKET EXPLORER: API URL from config: {:?}, using: {}",
        config.api_url,
        api_url
    );

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
        Ok(data) => match Manifest::decode(&data) {
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

    let return_url = format!("/buckets/{}", bucket_id);

    let template = BucketExplorerTemplate {
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
        current_path,
        path_segments,
        parent_path_url,
        items: display_items,
        read_only,
        viewing_history,
        at_hash: query.at,
        return_url,
        api_url,
    };

    template.into_response()
}

fn build_path_segments(path: &str) -> Vec<PathSegment> {
    if path == "/" {
        return vec![];
    }

    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
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

fn build_parent_path_url(current_path: &str, bucket_id: &Uuid, at_hash: Option<&String>) -> String {
    if current_path == "/" {
        return if let Some(hash) = at_hash {
            format!("/buckets/{}?at={}", bucket_id, hash)
        } else {
            format!("/buckets/{}", bucket_id)
        };
    }

    let parent = std::path::Path::new(current_path)
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
