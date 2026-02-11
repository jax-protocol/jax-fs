use axum::body::Body;
use axum::extract::DefaultBodyLimit;
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Router};
use http::header::{ACCEPT, ORIGIN};
use http::Method;
use rust_embed::RustEmbed;
use tokio::sync::watch;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tower_http::trace::{DefaultOnFailure, DefaultOnResponse};
use tower_http::LatencyUnit;

pub mod api;
mod config;
mod gateway_index;
mod handlers;
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

/// Run the API HTTP server (private, serves /_status + /api routes).
pub async fn run_api(
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

    let router = Router::new()
        .nest(STATUS_PREFIX, health::router(state.clone()))
        .nest(API_PREFIX, api::router(state.clone()))
        .fallback(handlers::not_found_handler)
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_SIZE_BYTES))
        .layer(Extension(config.clone()))
        .with_state(state)
        .layer(trace_layer);

    tracing::info!(addr = ?listen_addr, "API server listening");
    let listener = tokio::net::TcpListener::bind(listen_addr).await?;

    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.changed().await;
        })
        .await?;

    Ok(())
}

/// Run the gateway HTTP server (public, serves /_status + /gw + / + /static routes).
pub async fn run_gateway(
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

    // Gateway CORS (GET only) for gateway routes
    let gateway_cors = CorsLayer::new()
        .allow_methods(vec![Method::GET])
        .allow_headers(vec![ACCEPT, ORIGIN])
        .allow_origin(Any)
        .allow_credentials(false);

    // Gateway routes with their own CORS layer
    let gateway_routes = Router::new()
        .route("/:bucket_id", get(html::gateway::root_handler))
        .route("/:bucket_id/", get(html::gateway::root_handler))
        .route("/:bucket_id/*file_path", get(html::gateway::handler))
        .with_state(state.clone())
        .layer(gateway_cors);

    let router = Router::new()
        .nest(STATUS_PREFIX, health::router(state.clone()))
        .nest("/gw", gateway_routes)
        .route("/", get(gateway_index::handler))
        .route("/static/*path", get(static_handler))
        .fallback(handlers::not_found_handler)
        .layer(Extension(config.clone()))
        .with_state(state)
        .layer(trace_layer);

    tracing::info!(addr = ?listen_addr, "Gateway server listening");
    let listener = tokio::net::TcpListener::bind(listen_addr).await?;

    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.changed().await;
        })
        .await?;

    Ok(())
}

mod health;

#[derive(Debug, thiserror::Error)]
pub enum HttpServerError {
    #[error("an error occurred running the HTTP server: {0}")]
    ServingFailed(#[from] std::io::Error),
}
