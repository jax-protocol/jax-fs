use axum::Router;

pub mod bucket;
#[cfg(feature = "fuse")]
pub mod mounts;

use crate::ServiceState;

pub fn router(state: ServiceState) -> Router<ServiceState> {
    let router = Router::new().nest("/bucket", bucket::router(state.clone()));

    #[cfg(feature = "fuse")]
    let router = router.nest("/mounts", mounts::router(state.clone()));

    router.with_state(state)
}
