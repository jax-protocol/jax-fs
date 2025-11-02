use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::{Path, State};
use tokio::time::{timeout, Duration};
use tracing::instrument;
use uuid::Uuid;

use common::crypto::PublicKey;
// FIXME: ping_peer and SyncStatus don't exist yet in common::peer
// use common::peer::{ping_peer, NodeAddr, SyncStatus};
use crate::database::models::SyncStatus;

use crate::mount_ops;
use crate::ServiceState;

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
    pub total_peers: usize,
}

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub public_key: String,
    pub public_key_short: String,
    pub role: String,
    pub status: String,
    pub status_class: String,
}

#[instrument(skip(state))]
pub async fn handler(
    State(state): State<ServiceState>,
    Path(bucket_id): Path<Uuid>,
) -> askama_axum::Response {
    // Get bucket info
    let bucket = match mount_ops::get_bucket_info(bucket_id, &state).await {
        Ok(bucket) => bucket,
        Err(e) => return error_response(&format!("Failed to load bucket: {}", e)),
    };

    // Get bucket shares using mount_ops
    let shares = match mount_ops::get_bucket_shares(bucket_id, &state).await {
        Ok(shares) => shares,
        Err(e) => {
            tracing::error!("Failed to get bucket shares: {}", e);
            return error_response("Failed to load bucket shares");
        }
    };

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
        let peer_status = ("Unknown".to_string(), "bg-gray-100 text-gray-800".to_string());

        peers.push(PeerInfo {
            public_key: share.public_key.clone(),
            public_key_short: truncate_string(&share.public_key, 16),
            role: share.role,
            status: peer_status.0,
            status_class: peer_status.1,
        });
    }

    let total_peers = peers.len();

    let template = PeersExplorerTemplate {
        bucket_id: bucket_id.to_string(),
        bucket_name: bucket.name,
        peers,
        total_peers,
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
