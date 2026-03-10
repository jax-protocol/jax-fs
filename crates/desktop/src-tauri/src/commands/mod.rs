//! IPC command modules
//!
//! These modules expose Tauri IPC commands that access ServiceState directly.
//! A few commands (create, share, ping) still use HTTP for complex API flows.

pub mod bucket;
pub mod daemon;
pub mod logs;
pub mod mount;
