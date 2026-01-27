//! Shared test utilities for mount integration tests
#![allow(dead_code)]

use common::crypto::SecretKey;
use common::mount::Mount;
use common::peer::BlobsStore;
use tempfile::TempDir;
use uuid::Uuid;

/// Set up a test environment with a new mount, blob store, and owner key
pub async fn setup_test_env() -> (Mount, BlobsStore, SecretKey, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let blob_path = temp_dir.path().join("blobs");

    let secret_key = SecretKey::generate();
    let blobs = BlobsStore::fs(&blob_path).await.unwrap();

    let mount = Mount::init(Uuid::new_v4(), "test".to_string(), &secret_key, &blobs)
        .await
        .unwrap();

    (mount, blobs, secret_key, temp_dir)
}

/// Fork a mount by adding a new owner and having them load from the saved state.
/// Returns the new mount and its owner key.
pub async fn fork_mount(mount: &mut Mount, blobs: &BlobsStore) -> (Mount, SecretKey) {
    let new_key = SecretKey::generate();
    mount.add_owner(new_key.public()).await.unwrap();
    let (link, _, _) = mount.save(blobs, false).await.unwrap();
    let forked = Mount::load(&link, &new_key, blobs).await.unwrap();
    (forked, new_key)
}
