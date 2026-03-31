use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::State;
use uuid::Uuid;

use common::bucket_log::BucketLogProvider;

use crate::ServiceState;

/// Bucket display info for the gateway homepage.
#[derive(Debug, Clone)]
pub struct BucketDisplayInfo {
    pub id: Uuid,
    pub id_short: String,
    pub name: String,
    pub is_published: bool,
    pub version_short: String,
}

#[derive(Template)]
#[template(path = "pages/gateway/index.html")]
pub struct GatewayIndexTemplate {
    pub node_id: String,
    pub buckets: Vec<BucketDisplayInfo>,
}

/// Root page handler for the gateway.
/// Displays the gateway's public identity and available buckets.
pub async fn handler(State(state): State<ServiceState>) -> askama_axum::Response {
    let node_id = state.peer().id().to_string();

    let db_buckets = state
        .database()
        .list_buckets(None, None)
        .await
        .unwrap_or_default();

    // Only include buckets that have a published version
    let mut buckets = Vec::new();
    for (b, _status) in db_buckets {
        if let Ok(Some((link, _height))) = state.peer().logs().latest_published(b.id).await {
            let id_str = b.id.to_string();
            let id_short = format!("{}...{}", &id_str[..8], &id_str[id_str.len() - 4..]);

            let link_str = link.hash().to_string();
            let version_short = format!("{}...{}", &link_str[..8], &link_str[link_str.len() - 4..]);

            buckets.push(BucketDisplayInfo {
                id: b.id,
                id_short,
                name: b.name,
                is_published: true,
                version_short,
            });
        }
    }

    let template = GatewayIndexTemplate { node_id, buckets };

    template.into_response()
}
