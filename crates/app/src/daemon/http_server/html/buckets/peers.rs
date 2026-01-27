use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::{Path, State};
use axum::Extension;
use tracing::instrument;
use uuid::Uuid;

use common::crypto::PublicKey;
use common::linked_data::BlockEncoded;
use common::mount::PrincipalRole;
use common::peer::PingReplyStatus;
use common::prelude::Manifest;

use crate::daemon::http_server::Config;
use crate::ServiceState;

use super::file_explorer::{FileMetadata, PathSegment};

#[derive(Debug, Clone)]
pub struct ShareInfo {
    pub public_key: String,
    pub role: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ManifestShare {
    pub public_key: String,
    pub role: String,
}

/// Truncate a string to a maximum length, adding "..." if truncated
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}

/// Get status badge styling for a given sync status
fn status_badge_class(status: &PingReplyStatus) -> (&'static str, &'static str) {
    match status {
        PingReplyStatus::NotFound => ("Not Found", "bg-gray-100 text-gray-800"),
        PingReplyStatus::Behind(_, _) => ("Behind", "bg-yellow-100 text-yellow-800"),
        PingReplyStatus::InSync => ("In Sync", "bg-green-100 text-green-800"),
        PingReplyStatus::Ahead(_, _) => ("Ahead", "bg-orange-100 text-orange-800"),
    }
}

#[derive(Template)]
#[template(path = "pages/buckets/peers.html")]
pub struct PeersExplorerTemplate {
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
    pub peers: Vec<PeerInfo>,
    pub peers_json: String,
    pub total_peers: usize,
    pub api_url: String,
    pub current_path: String,
    pub file_metadata: Option<FileMetadata>,
    pub path_segments: Vec<PathSegment>,
    pub is_published: bool,
    /// Our node's role for this bucket (Owner, Mirror, or None if not a share)
    pub our_role: Option<PrincipalRole>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PeerInfo {
    pub public_key: String,
    pub public_key_short: String,
    pub role: String,
    pub status: String,
    pub status_class: String,
}

#[instrument(skip(state, config))]
pub async fn handler(
    State(state): State<ServiceState>,
    Extension(config): Extension<Config>,
    Path(bucket_id): Path<Uuid>,
) -> askama_axum::Response {
    // Get bucket info from bucket_log
    let bucket = match state.database().get_bucket_info(&bucket_id).await {
        Ok(Some(bucket)) => bucket,
        Ok(None) => return error_response("Bucket not found"),
        Err(e) => return error_response(&format!("Failed to load bucket: {}", e)),
    };

    // Load mount and get bucket shares from manifest
    // (owners see HEAD, mirrors see latest_published)
    let mount = match state.peer().mount_for_read(bucket_id).await {
        Ok(mount) => mount,
        Err(e) => {
            tracing::error!("Failed to load mount: {}", e);
            return error_response("Failed to load bucket");
        }
    };

    let manifest = mount.inner().await.manifest().clone();

    // Get our role from the manifest
    let our_public_key = state.peer().secret().public();
    let our_role = manifest
        .get_share(&our_public_key)
        .map(|s| s.role().clone());

    let shares: Vec<ShareInfo> = manifest
        .shares()
        .iter()
        .map(|(key, share)| ShareInfo {
            public_key: key.clone(),
            role: format!("{:?}", share.principal().role),
        })
        .collect();

    // Get our node ID to filter ourselves out
    let our_node_id = state.node().id();
    let our_node_id_hex = our_node_id.to_string();

    // Ping all peers and collect responses (with 5 second timeout)
    let ping_results = match state
        .peer()
        .ping_and_collect(bucket_id, Some(std::time::Duration::from_secs(5)))
        .await
    {
        Ok(results) => results,
        Err(e) => {
            tracing::error!("Failed to ping peers: {}", e);
            Default::default()
        }
    };

    // Build peer list with status
    let mut peers = Vec::new();
    for share in shares {
        // Skip ourselves by comparing hex strings
        if share.public_key == our_node_id_hex {
            tracing::debug!("Skipping self from peers list: {}", share.public_key);
            continue;
        }

        // Parse the public key from hex
        let _pub_key = match PublicKey::from_hex(&share.public_key) {
            Ok(key) => key,
            Err(e) => {
                tracing::error!("Invalid public key {}: {}", share.public_key, e);
                continue; // Skip invalid keys
            }
        };

        // Get ping status for this peer
        let peer_status = match ping_results.get(&share.public_key) {
            Some(status) => {
                let (label, class) = status_badge_class(status);
                (label.to_string(), class.to_string())
            }
            None => (
                "Unknown".to_string(),
                "bg-gray-100 text-gray-800".to_string(),
            ),
        };

        peers.push(PeerInfo {
            public_key: share.public_key.clone(),
            public_key_short: truncate_string(&share.public_key, 16),
            role: share.role,
            status: peer_status.0,
            status_class: peer_status.1,
        });
    }

    let total_peers = peers.len();

    let api_url = config
        .api_url
        .clone()
        .unwrap_or_else(|| "http://localhost:5000".to_string());

    let peers_json = serde_json::to_string(&peers).unwrap_or_else(|_| "[]".to_string());

    // Format bucket link for display
    let bucket_link = bucket.link.hash().to_string();
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
        is_published,
    ) = match blobs.get(&bucket.link.hash()).await {
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
                let published = bucket_data.is_published();

                (
                    formatted, height, version, entry_link, pins_link, previous, shares, published,
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
                    false,
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
                false,
            )
        }
    };

    let template = PeersExplorerTemplate {
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
        peers,
        peers_json,
        total_peers,
        api_url,
        current_path: "/".to_string(),
        file_metadata: None,
        path_segments: vec![],
        is_published,
        our_role,
    };

    template.into_response()
}

fn error_response(message: &str) -> askama_axum::Response {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        format!("Error: {}", message),
    )
        .into_response()
}
