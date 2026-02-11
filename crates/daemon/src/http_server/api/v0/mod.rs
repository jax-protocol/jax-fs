use axum::Router;

pub mod bucket;

use crate::ServiceState;

pub fn router(state: ServiceState) -> Router<ServiceState> {
    Router::new()
        .nest("/bucket", bucket::router(state.clone()))
        .with_state(state)
}
