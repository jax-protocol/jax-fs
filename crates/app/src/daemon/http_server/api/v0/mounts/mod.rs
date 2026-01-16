//! Mounts API endpoints
//!
//! REST API for managing FUSE mount configurations.

use axum::routing::{delete, get, patch, post};
use axum::Router;

use crate::ServiceState;

pub mod create;
pub mod delete_mount;
mod get_mount;
pub mod list;
pub mod start;
pub mod stop;
pub mod update;

pub use create::{CreateMountRequest, CreateMountResponse};
pub use list::{ListMountsRequest, ListMountsResponse, MountInfoResponse};

pub fn router(state: ServiceState) -> Router<ServiceState> {
    Router::new()
        .route("/", post(create::handler))
        .route("/", get(list::handler))
        .route("/:mount_id", get(get_mount::handler))
        .route("/:mount_id", patch(update::handler))
        .route("/:mount_id", delete(delete_mount::handler))
        .route("/:mount_id/start", post(start::handler))
        .route("/:mount_id/stop", post(stop::handler))
        .with_state(state)
}
