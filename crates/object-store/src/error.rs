//! Error types for the blobs store.

use std::path::PathBuf;

/// Errors that can occur when working with the blob store.
#[derive(Debug, thiserror::Error)]
pub enum BlobStoreError {
    /// Database error
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Object storage error
    #[error("object storage error: {0}")]
    ObjectStore(#[from] object_store::Error),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Migration error
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    /// Hash parse error
    #[error("invalid hash: {0}")]
    InvalidHash(String),

    /// Blob not found
    #[error("blob not found: {0}")]
    NotFound(String),

    /// Invalid hash list format
    #[error("invalid hash list: {0}")]
    InvalidHashList(String),

    /// Invalid configuration
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    /// Path error
    #[error("path error: {0}")]
    Path(PathBuf),

    /// S3 bucket not found - must be created before use
    #[error("S3 bucket '{0}' does not exist. Create it before starting the node.")]
    BucketNotFound(String),
}

/// Result type alias for blob store operations.
pub type Result<T> = std::result::Result<T, BlobStoreError>;
