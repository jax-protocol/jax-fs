use axum::Router;

pub mod bucket;

use service::ServiceState;

pub fn router(state: ServiceState) -> Router<ServiceState> {
    Router::new()
        .nest("/bucket", bucket::router(state.clone()))
        .with_state(state)
}
