use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Json;
use http::StatusCode;
use serde::Serialize;

use crate::ServiceState;

#[derive(Serialize)]
pub struct IdentityResponse {
    /// The node's public identity (NodeId)
    pub node_id: String,
}

#[tracing::instrument(skip(state))]
pub async fn handler(State(state): State<ServiceState>) -> Response {
    let node_id = state.peer().id().to_string();
    (StatusCode::OK, Json(IdentityResponse { node_id })).into_response()
}
