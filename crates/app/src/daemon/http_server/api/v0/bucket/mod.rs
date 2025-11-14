use axum::routing::post;
use axum::Router;

use crate::ServiceState;

pub mod add;
pub mod cat;
pub mod create;
pub mod list;
pub mod ls;
pub mod ping;
pub mod share;

// Re-export for convenience
pub use create::CreateRequest;
pub use list::ListRequest;
pub use share::ShareRequest;

pub fn router(state: ServiceState) -> Router<ServiceState> {
    Router::new()
        .route("/", post(create::handler))
        .route("/list", post(list::handler))
        .route("/add", post(add::handler))
        .route("/ls", post(ls::handler))
        .route("/cat", post(cat::handler))
        .route("/ping", post(ping::handler))
        .route("/share", post(share::handler))
        .with_state(state)
}
