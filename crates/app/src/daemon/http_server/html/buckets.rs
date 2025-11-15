use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::Extension;
use tracing::instrument;

use crate::daemon::http_server::Config;
use crate::ServiceState;

#[derive(Template)]
#[template(path = "pages/index.html")]
pub struct BucketsTemplate {
    pub buckets: Vec<BucketDisplayInfo>,
    pub read_only: bool,
    pub api_url: String,
    pub peer_id: String,
    pub peer_id_short: String,
}

#[derive(Debug, Clone)]
pub struct BucketDisplayInfo {
    pub bucket_id: String,
    pub bucket_name: String,
    pub bucket_icon: String,
    pub file_count: usize,
    pub peer_count: usize,
    pub last_modified: String,
}

#[axum::debug_handler]
#[instrument(skip(state, config))]
pub async fn handler(
    State(state): State<ServiceState>,
    Extension(config): Extension<Config>,
    _headers: HeaderMap,
) -> askama_axum::Response {
    // Use the read_only flag from config
    let read_only = config.read_only;

    // Load buckets from database
    let buckets = match state.database().list_buckets(None, None).await {
        Ok(buckets) => buckets,
        Err(e) => {
            tracing::error!("Failed to list buckets: {}", e);
            return error_response("Failed to load buckets");
        }
    };

    // Convert to display format with file/peer counts
    let mut display_buckets: Vec<BucketDisplayInfo> = Vec::new();

    for b in buckets {
        // Get first letter of bucket name for icon
        let bucket_icon = b
            .name
            .chars()
            .next()
            .unwrap_or('B')
            .to_uppercase()
            .to_string();

        // Try to load mount to get file/peer counts
        let (file_count, peer_count) = match state.peer().mount(b.id).await {
            Ok(mount) => {
                let inner = mount.inner().await;

                // Count files recursively
                let file_count =
                    match count_files_recursive(&mount, &std::path::PathBuf::from("/")).await {
                        Ok(count) => count,
                        Err(e) => {
                            tracing::warn!("Failed to count files for bucket {}: {}", b.id, e);
                            0
                        }
                    };

                // Get peer count from manifest shares
                let manifest = inner.manifest();
                let peer_count = manifest.shares().len();

                (file_count, peer_count)
            }
            Err(e) => {
                tracing::warn!("Failed to load mount for bucket {}: {}", b.id, e);
                (0, 0)
            }
        };

        display_buckets.push(BucketDisplayInfo {
            bucket_id: b.id.to_string(),
            bucket_name: b.name,
            bucket_icon,
            file_count,
            peer_count,
            last_modified: format_relative_time(b.created_at),
        });
    }

    let api_url = config
        .api_url
        .clone()
        .unwrap_or_else(|| "http://localhost:3000".to_string());

    tracing::info!(
        "BUCKETS PAGE: API URL from config: {:?}, using: {}",
        config.api_url,
        api_url
    );

    // Get our peer ID
    let peer_id = state.node().id().to_string();
    // Mobile: show fewer characters (8...8)
    let peer_id_short = if peer_id.len() > 16 {
        format!("{}...{}", &peer_id[..8], &peer_id[peer_id.len() - 8..])
    } else {
        peer_id.clone()
    };

    let template = BucketsTemplate {
        buckets: display_buckets,
        read_only,
        api_url,
        peer_id,
        peer_id_short,
    };

    template.into_response()
}

/// Type alias for the recursive file counting future
type FileCountFuture<'a> = std::pin::Pin<
    Box<
        dyn std::future::Future<Output = Result<usize, Box<dyn std::error::Error + Send + Sync>>>
            + Send
            + 'a,
    >,
>;

/// Count files recursively in a mount
fn count_files_recursive<'a>(
    mount: &'a common::mount::Mount,
    path: &'a std::path::Path,
) -> FileCountFuture<'a> {
    Box::pin(async move {
        let items = mount.ls(path).await?;
        let mut count = 0;

        for (item_path, node_link) in items {
            match node_link {
                common::mount::NodeLink::Dir(..) => {
                    // Recursively count files in directories
                    let full_path = path.join(&item_path);
                    count += count_files_recursive(mount, &full_path).await?;
                }
                common::mount::NodeLink::Data(..) => {
                    count += 1;
                }
            }
        }

        Ok(count)
    })
}

/// Format timestamp as relative time (Today, Yesterday, or date)
fn format_relative_time(ts: time::OffsetDateTime) -> String {
    let now = time::OffsetDateTime::now_utc();
    let diff = now - ts;

    if diff.whole_days() == 0 {
        "Today".to_string()
    } else if diff.whole_days() == 1 {
        "Yesterday".to_string()
    } else if diff.whole_days() < 7 {
        format!("{} days ago", diff.whole_days())
    } else {
        // Format as "Nov 14"
        ts.format(&time::format_description::parse("[month repr:short] [day]").unwrap())
            .unwrap_or_else(|_| ts.to_string())
    }
}

fn error_response(message: &str) -> askama_axum::Response {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        format!("Error: {}", message),
    )
        .into_response()
}
