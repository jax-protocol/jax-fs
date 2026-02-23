//! Desktop-specific build information.
//!
//! This module captures build-time information for the desktop crate,
//! ensuring CARGO_PKG_VERSION comes from desktop's compile environment.

use common::version::BuildInfo;

/// Create build info using desktop's compile-time environment.
pub fn build_info() -> BuildInfo {
    BuildInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        git_hash: option_env!("REPO_VERSION").unwrap_or("unknown").to_string(),
        build_profile: option_env!("BUILD_PROFILE")
            .unwrap_or("unknown")
            .to_string(),
        build_features: option_env!("BUILD_FEATURES").unwrap_or("none").to_string(),
        build_timestamp: option_env!("BUILD_TIMESTAMP")
            .unwrap_or("unknown")
            .to_string(),
        rust_version: option_env!("RUST_VERSION").unwrap_or("unknown").to_string(),
        target: option_env!("BUILD_TARGET").unwrap_or("unknown").to_string(),
        host: option_env!("BUILD_HOST").unwrap_or("unknown").to_string(),
    }
}
