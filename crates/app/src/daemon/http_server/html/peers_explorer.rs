use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::{Path, State};
use axum::Extension;
use tracing::instrument;
use uuid::Uuid;

use common::crypto::PublicKey;
// FIXME: ping_peer and SyncStatus don't exist yet in common::peer
// use common::peer::{ping_peer, NodeAddr, SyncStatus};

use crate::daemon::http_server::Config;
use crate::ServiceState;

#[derive(Debug, Clone)]
pub struct ShareInfo {
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
// FIXME: Commented out until SyncStatus is properly defined
// fn status_badge_class(status: &SyncStatus) -> (&'static str, &'static str) {
//     match status {
//         SyncStatus::NotFound => ("Not Found", "bg-gray-100 text-gray-800"),
//         SyncStatus::Behind => ("Behind", "bg-yellow-100 text-yellow-800"),
//         SyncStatus::InSync => ("In Sync", "bg-green-100 text-green-800"),
//         SyncStatus::Ahead => ("Ahead", "bg-orange-100 text-orange-800"),
//     }
// }

#[derive(Template)]
#[template(path = "peers_explorer.html")]
pub struct PeersExplorerTemplate {
    pub bucket_id: String,
    pub bucket_name: String,
    pub peers: Vec<PeerInfo>,
    pub peers_json: String,
    pub total_peers: usize,
    pub api_url: String,
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
    let mount = match state.peer().mount(bucket_id).await {
        Ok(mount) => mount,
        Err(e) => {
            tracing::error!("Failed to load mount: {}", e);
            return error_response("Failed to load bucket");
        }
    };

    let manifest = mount.inner().await.manifest().clone();
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

    // Ping each peer to check their status (excluding ourselves)
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

        // FIXME: ping_peer and NodeAddr don't exist yet
        // Temporarily stub out peer pinging functionality
        let peer_status = (
            "Unknown".to_string(),
            "bg-gray-100 text-gray-800".to_string(),
        );

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

    let template = PeersExplorerTemplate {
        bucket_id: bucket_id.to_string(),
        bucket_name: bucket.name,
        peers,
        peers_json,
        total_peers,
        api_url,
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
