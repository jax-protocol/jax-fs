//! Import operations for the object store + SQLite blob store.
//!
//! Handles importing bytes into object storage and updating SQLite metadata.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use bao_tree::{blake3, io::outboard::PreOrderMemOutboard};
use bytes::Bytes;
use iroh_blobs::Hash;
use tracing::{debug, info};

use super::bao_file::IROH_BLOCK_SIZE;
use super::entry_state::needs_outboard;
use super::minio::BlobObjectStore;
use crate::daemon::database::Database;

/// Result of an import operation.
pub struct ImportResult {
    pub hash: Hash,
    pub size: u64,
}

/// Import bytes into the store.
pub async fn import_bytes(
    data: Bytes,
    store: Arc<BlobObjectStore>,
    db: Database,
) -> Result<ImportResult> {
    let size = data.len() as u64;

    // Compute the BLAKE3 hash
    let hash_bytes = blake3::hash(&data);
    let hash = Hash::from(*hash_bytes.as_bytes());
    let hash_str = hash.to_hex().to_string();

    debug!("importing {} bytes, hash={}", size, hash_str);

    // Compute outboard if needed
    let outboard_data = if needs_outboard(size) {
        // Create the outboard using bao-tree
        let outboard = PreOrderMemOutboard::create(&data, IROH_BLOCK_SIZE);
        Some(outboard.data.to_vec())
    } else {
        None
    };

    // Store data in object storage
    store.put_data(&hash_str, data).await?;

    // Store outboard in object storage if present
    let has_outboard = if let Some(ob) = outboard_data {
        store.put_outboard(&hash_str, Bytes::from(ob)).await?;
        true
    } else {
        false
    };

    // Update SQLite metadata
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    sqlx::query(
        r#"
        INSERT INTO blobs (hash, size, has_outboard, state, created_at, updated_at)
        VALUES (?, ?, ?, 'complete', ?, ?)
        ON CONFLICT(hash) DO UPDATE SET
            size = excluded.size,
            has_outboard = excluded.has_outboard,
            state = 'complete',
            updated_at = excluded.updated_at
        "#,
    )
    .bind(&hash_str)
    .bind(size as i64)
    .bind(has_outboard)
    .bind(now)
    .bind(now)
    .execute(&*db)
    .await?;

    info!(
        "stored blob {} ({} bytes) in object storage and SQLite",
        hash_str, size
    );

    Ok(ImportResult { hash, size })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_needs_outboard() {
        assert!(!needs_outboard(0));
        assert!(!needs_outboard(1024));
        assert!(!needs_outboard(16 * 1024));
        assert!(needs_outboard(16 * 1024 + 1));
    }
}
