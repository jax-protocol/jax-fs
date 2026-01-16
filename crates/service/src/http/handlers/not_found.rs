use askama::Template;
use askama_axum::IntoResponse;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::Json;

#[derive(Template)]
#[template(path = "pages/not_found.html")]
struct NotFoundTemplate {}

pub async fn not_found_handler(headers: HeaderMap) -> Response {
    let accept = headers
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok());

    match accept {
        Some(accept_str) if accept_str.contains("application/json") => {
            let err_msg = serde_json::json!({"msg": "not found"});
            (StatusCode::NOT_FOUND, Json(err_msg)).into_response()
        }
        Some(accept_str) if accept_str.contains("text/html") => {
            let template = NotFoundTemplate {};
            (StatusCode::NOT_FOUND, template).into_response()
        }
        _ => (
            StatusCode::NOT_FOUND,
            [(axum::http::header::CONTENT_TYPE, "text/plain")],
            "not found",
        )
            .into_response(),
    }
}
