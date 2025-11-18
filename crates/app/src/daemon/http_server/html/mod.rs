use axum::routing::get;
use axum::Router;
use http::header::{ACCEPT, ORIGIN};
use http::Method;
use tower_http::cors::{Any, CorsLayer};

mod buckets;
mod gateway;
mod index;

use crate::ServiceState;

pub fn router(state: ServiceState) -> Router<ServiceState> {
    let cors_layer = CorsLayer::new()
        .allow_methods(vec![Method::GET])
        .allow_headers(vec![ACCEPT, ORIGIN])
        .allow_origin(Any)
        .allow_credentials(false);

    Router::new()
        .route("/", get(index::handler))
        .route("/buckets", get(index::handler))
        .route("/buckets/:bucket_id", get(buckets::file_explorer::handler))
        .route(
            "/buckets/:bucket_id/view",
            get(buckets::file_viewer::handler),
        )
        .route(
            "/buckets/:bucket_id/edit",
            get(buckets::file_editor::handler),
        )
        .route("/buckets/:bucket_id/logs", get(buckets::history::handler))
        .route("/buckets/:bucket_id/peers", get(buckets::peers::handler))
        .route("/gw/:bucket_id/*file_path", get(gateway::handler))
        .with_state(state)
        .layer(cors_layer)
}
