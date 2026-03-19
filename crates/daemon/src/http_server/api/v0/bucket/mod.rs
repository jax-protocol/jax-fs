use axum::routing::post;
use axum::Router;

use crate::ServiceState;

pub mod add;
pub mod approve;
pub mod cat;
pub mod create;
pub mod delete;
pub mod export;
pub mod history;
pub mod ignore;
pub mod latest_published;
pub mod list;
pub mod ls;
pub mod mkdir;
pub mod mv;
pub mod ping;
pub mod publish;
pub mod rename;
pub mod share;
pub mod shares;
pub mod stat;
pub mod unpublish;
pub mod unshare;
pub mod update;

// Re-export for convenience
pub use approve::ApproveRequest;
pub use create::CreateRequest;
pub use ignore::IgnoreRequest;
pub use list::ListRequest;
pub use share::ShareRequest;
pub use shares::SharesRequest;
pub use unshare::UnshareRequest;

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
        .route("/shares", post(shares::handler))
        .route("/unshare", post(unshare::handler))
        .route("/publish", post(publish::handler))
        .route("/unpublish", post(unpublish::handler))
        .route("/export", post(export::handler))
        .route("/latest-published", post(latest_published::handler))
        .route("/history", post(history::handler))
        .route("/stat", post(stat::handler))
        .route("/approve", post(approve::handler))
        .route("/ignore", post(ignore::handler))
        .with_state(state)
}
