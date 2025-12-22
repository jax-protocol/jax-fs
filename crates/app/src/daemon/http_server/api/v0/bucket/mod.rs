use axum::routing::post;
use axum::Router;

use crate::ServiceState;

pub mod add;
pub mod cat;
pub mod create;
pub mod delete;
pub mod export;
pub mod list;
pub mod ls;
pub mod mkdir;
pub mod mv;
pub mod ping;
pub mod rename;
pub mod share;
pub mod update;

// Re-export for convenience
pub use create::CreateRequest;
pub use list::ListRequest;
pub use share::ShareRequest;

pub fn router(state: ServiceState) -> Router<ServiceState> {
    Router::new()
        .route("/", post(create::handler))
        .route("/list", post(list::handler))
        .route("/add", post(add::handler))
        .route("/update", post(update::handler))
        .route("/rename", post(rename::handler))
        .route("/mv", post(mv::handler))
        .route("/delete", post(delete::handler))
        .route("/mkdir", post(mkdir::handler))
        .route("/ls", post(ls::handler))
        .route("/cat", post(cat::handler).get(cat::handler_get))
        .route("/ping", post(ping::handler))
        .route("/share", post(share::handler))
        .route("/export", post(export::handler))
        .with_state(state)
}
