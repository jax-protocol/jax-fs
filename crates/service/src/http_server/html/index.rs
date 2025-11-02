use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::State;
use tracing::instrument;

use crate::ServiceState;

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub node_id: String,
    pub blobs_path: String,
}

#[instrument(skip(state))]
pub async fn handler(State(state): State<ServiceState>) -> askama_axum::Response {
    let node = state.node();
    let node_id = node.id().to_string();
    // FIXME: blobs_store_path method doesn't exist yet on Peer
    let blobs_path = "N/A".to_string();

    let template = IndexTemplate {
        node_id,
        blobs_path,
    };

    template.into_response()
}
