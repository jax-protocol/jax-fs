//! HTTP handlers and routers for the service.

pub mod config;
pub mod handlers;
pub mod health;
pub mod html;

pub use config::Config;
pub use handlers::not_found_handler;

/// Maximum upload size in bytes (500 MB)
pub const MAX_UPLOAD_SIZE_BYTES: usize = 500 * 1024 * 1024;
