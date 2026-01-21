use axum::body::Body;
use axum::extract::DefaultBodyLimit;
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Router};
use rust_embed::RustEmbed;
use tokio::sync::watch;
use tower_http::trace::TraceLayer;
use tower_http::trace::{DefaultOnFailure, DefaultOnResponse};
use tower_http::LatencyUnit;

pub mod api;
mod config;
mod gateway_index;
mod handlers;
mod health;
mod html;

pub use config::Config;

use crate::ServiceState;

const API_PREFIX: &str = "/api";
const STATUS_PREFIX: &str = "/_status";

/// Maximum upload size in bytes (500 MB)
pub const MAX_UPLOAD_SIZE_BYTES: usize = 500 * 1024 * 1024;

#[derive(RustEmbed)]
#[folder = "static"]
struct StaticAssets;

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri
        .path()
        .trim_start_matches('/')
        .trim_start_matches("static/");

    match StaticAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(content.data.to_vec()))
                .unwrap()
        }
        None => {
            // Serve 404.html if file not found
            match StaticAssets::get("404.html") {
                Some(content) => Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .header(header::CONTENT_TYPE, "text/html")
                    .body(Body::from(content.data.to_vec()))
                    .unwrap(),
                None => Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::from("Not Found"))
                    .unwrap(),
            }
        }
    }
}

/// Run the combined App server (UI + API on same port)
pub async fn run_app(
    config: Config,
    state: ServiceState,
    mut shutdown_rx: watch::Receiver<()>,
) -> Result<(), HttpServerError> {
    let listen_addr = config.listen_addr;
    let log_level = config.log_level;
    let trace_layer = TraceLayer::new_for_http()
        .on_response(
            DefaultOnResponse::new()
                .include_headers(false)
                .level(log_level)
                .latency_unit(LatencyUnit::Micros),
        )
        .on_failure(DefaultOnFailure::new().latency_unit(LatencyUnit::Micros));

    tracing::info!("Static files embedded in binary");

    // Combined router with both HTML UI and API endpoints
    let app_router = Router::new()
        .nest(STATUS_PREFIX, health::router(state.clone()))
        .nest(API_PREFIX, api::router(state.clone()))
        .route("/static/*path", axum::routing::get(static_handler))
        .merge(html::router(state.clone()))
        .fallback(handlers::not_found_handler)
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_SIZE_BYTES))
        .layer(Extension(config.clone()))
        .with_state(state)
        .layer(trace_layer);

    tracing::info!(addr = ?listen_addr, "App server listening (UI + API)");
    let listener = tokio::net::TcpListener::bind(listen_addr).await?;

    axum::serve(listener, app_router)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.changed().await;
        })
        .await?;

    Ok(())
}

/// Run a minimal gateway-only HTTP server.
/// Only serves /gw/:bucket_id/*file_path and health endpoints.
/// No Askama UI routes, no REST API routes.
pub async fn run_gateway(
    config: Config,
    state: ServiceState,
    mut shutdown_rx: watch::Receiver<()>,
) -> Result<(), HttpServerError> {
    use axum::routing::get;
    use http::header::{ACCEPT, ORIGIN};
    use http::Method;
    use tower_http::cors::{Any, CorsLayer};

    let listen_addr = config.listen_addr;
    let log_level = config.log_level;
    let trace_layer = TraceLayer::new_for_http()
        .on_response(
            DefaultOnResponse::new()
                .include_headers(false)
                .level(log_level)
                .latency_unit(LatencyUnit::Micros),
        )
        .on_failure(DefaultOnFailure::new().latency_unit(LatencyUnit::Micros));

    let cors_layer = CorsLayer::new()
        .allow_methods(vec![Method::GET])
        .allow_headers(vec![ACCEPT, ORIGIN])
        .allow_origin(Any)
        .allow_credentials(false);

    // Minimal router: root page, gateway route, static files, and health endpoints
    let gateway_router = Router::new()
        .route("/", get(gateway_index::handler))
        .route("/gw/:bucket_id", get(html::gateway::root_handler))
        .route("/gw/:bucket_id/", get(html::gateway::root_handler))
        .route("/gw/:bucket_id/*file_path", get(html::gateway::handler))
        .route("/static/*path", get(static_handler))
        .nest(STATUS_PREFIX, health::router(state.clone()))
        .fallback(handlers::not_found_handler)
        .with_state(state)
        .layer(cors_layer)
        .layer(trace_layer);

    tracing::info!(addr = ?listen_addr, "Gateway server listening");
    let listener = tokio::net::TcpListener::bind(listen_addr).await?;

    axum::serve(listener, gateway_router)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.changed().await;
        })
        .await?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum HttpServerError {
    #[error("an error occurred running the HTTP server: {0}")]
    ServingFailed(#[from] std::io::Error),
}
