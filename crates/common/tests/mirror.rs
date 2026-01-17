//! Integration tests for Mount mirror and publish operations

mod common;

use std::io::Cursor;
use std::path::PathBuf;

use ::common::crypto::SecretKey;
use ::common::mount::{Mount, MountError};
use ::common::peer::BlobsStore;
use tempfile::TempDir;

const TEST_PATH: &str = "/file.txt";

/// Set up a mount with content and a mirror for publish/mirror tests.
async fn setup_mount_with_mirror(
    content: &[u8],
) -> (Mount, BlobsStore, SecretKey, SecretKey, TempDir) {
    let (mut mount, blobs, owner_key, temp_dir) = common::setup_test_env().await;

    // Add content
    mount
        .add(&PathBuf::from(TEST_PATH), Cursor::new(content.to_vec()))
        .await
        .unwrap();

    // Add a mirror
    let mirror_key = SecretKey::generate();
    mount.add_mirror(mirror_key.public()).await;

    (mount, blobs, owner_key, mirror_key, temp_dir)
}

#[tokio::test]
async fn test_mirror_cannot_mount_unpublished_bucket() {
    let (mount, blobs, _, mirror_key, _temp) = setup_mount_with_mirror(b"secret data").await;

    // Save without publishing
    let (link, _, _) = mount.save(&blobs, false).await.unwrap();

    // Mirror should fail to mount unpublished bucket
    let result = Mount::load(&link, &mirror_key, &blobs).await;
    assert!(
        matches!(result, Err(MountError::MirrorCannotMount)),
        "Mirror should not be able to mount unpublished bucket"
    );
}

#[tokio::test]
async fn test_mirror_can_mount_published_bucket() {
    let (mount, blobs, _, mirror_key, _temp) = setup_mount_with_mirror(b"published data").await;

    // Publish grants decryption access to mirrors
    let (link, _, _) = mount.publish().await.unwrap();

    // Mirror can now mount
    let mirror_mount = Mount::load(&link, &mirror_key, &blobs)
        .await
        .expect("Mirror should be able to mount published bucket");

    let data = mirror_mount.cat(&PathBuf::from(TEST_PATH)).await.unwrap();
    assert_eq!(data, b"published data");
}

#[tokio::test]
async fn test_owner_can_always_mount() {
    let (mount, blobs, owner_key, _, _temp) = setup_mount_with_mirror(b"owner data").await;

    // Save without publishing
    let (link, _, _) = mount.save(&blobs, false).await.unwrap();

    // Owner can mount regardless of publish state
    let owner_mount = Mount::load(&link, &owner_key, &blobs)
        .await
        .expect("Owner should always be able to mount");

    let data = owner_mount.cat(&PathBuf::from(TEST_PATH)).await.unwrap();
    assert_eq!(data, b"owner data");
}
