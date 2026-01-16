use axum::Router;

pub mod bucket;
pub mod mounts;

use crate::ServiceState;

pub fn router(state: ServiceState) -> Router<ServiceState> {
    Router::new()
        .nest("/bucket", bucket::router(state.clone()))
        .nest("/mounts", mounts::router(state.clone()))
        .with_state(state)
}
