use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::State;

use crate::ServiceState;

#[derive(Template)]
#[template(path = "pages/gateway/index.html")]
pub struct GatewayIndexTemplate {
    pub node_id: String,
}

/// Root page handler for the gateway.
/// Displays the gateway's public identity (NodeId).
pub async fn handler(State(state): State<ServiceState>) -> askama_axum::Response {
    let node_id = state.peer().id().to_string();

    let template = GatewayIndexTemplate { node_id };

    template.into_response()
}
