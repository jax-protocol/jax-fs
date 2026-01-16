//! Integration tests for Mount operations

use std::io::Cursor;
use std::path::PathBuf;

use common::crypto::{Secret, SecretKey};
use common::mount::{Mount, MountError, PrincipalRole};
use common::peer::BlobsStore;
use tempfile::TempDir;
use uuid::Uuid;

#[tokio::test]
async fn test_mirror_cannot_mount_unpublished_bucket() {
    let temp_dir = TempDir::new().unwrap();
    let blob_path = temp_dir.path().join("blobs");
    let blobs = BlobsStore::fs(&blob_path).await.unwrap();

    // Owner creates a bucket
    let owner_key = SecretKey::generate();
    let mut mount = Mount::init(Uuid::new_v4(), "test".to_string(), &owner_key, &blobs)
        .await
        .unwrap();

    // Add some content
    mount
        .add(
            &PathBuf::from("/file.txt"),
            Cursor::new(b"secret data".to_vec()),
        )
        .await
        .unwrap();

    // Add a mirror peer (without publishing)
    let mirror_key = SecretKey::generate();
    mount
        .add_principal(mirror_key.public(), PrincipalRole::Mirror)
        .await
        .unwrap();

    // Save the mount
    let (link, _, _) = mount.save(&blobs).await.unwrap();

    // Mirror tries to mount - should fail because bucket is not published
    let result = Mount::load(&link, &mirror_key, &blobs).await;
    assert!(
        matches!(result, Err(MountError::MirrorCannotMount)),
        "Mirror should not be able to mount unpublished bucket"
    );
}

#[tokio::test]
async fn test_mirror_can_mount_published_bucket() {
    let temp_dir = TempDir::new().unwrap();
    let blob_path = temp_dir.path().join("blobs");
    let blobs = BlobsStore::fs(&blob_path).await.unwrap();

    // Owner creates a bucket
    let owner_key = SecretKey::generate();
    let mut mount = Mount::init(Uuid::new_v4(), "test".to_string(), &owner_key, &blobs)
        .await
        .unwrap();

    // Add some content
    mount
        .add(
            &PathBuf::from("/file.txt"),
            Cursor::new(b"published data".to_vec()),
        )
        .await
        .unwrap();

    // Add a mirror peer
    let mirror_key = SecretKey::generate();
    mount
        .add_principal(mirror_key.public(), PrincipalRole::Mirror)
        .await
        .unwrap();

    // Publish the bucket - this grants decryption access to mirrors
    let secret = Secret::generate();
    mount.publish(&secret).await.unwrap();

    // Save the mount
    let (link, _, _) = mount.save(&blobs).await.unwrap();

    // Mirror loads the mount - should succeed now that bucket is published
    let mirror_mount = Mount::load(&link, &mirror_key, &blobs)
        .await
        .expect("Mirror should be able to mount published bucket");

    // Mirror can read the content
    let data = mirror_mount.cat(&PathBuf::from("/file.txt")).await.unwrap();
    assert_eq!(data, b"published data");
}

#[tokio::test]
async fn test_owner_can_always_mount() {
    let temp_dir = TempDir::new().unwrap();
    let blob_path = temp_dir.path().join("blobs");
    let blobs = BlobsStore::fs(&blob_path).await.unwrap();

    // Owner creates a bucket
    let owner_key = SecretKey::generate();
    let mut mount = Mount::init(Uuid::new_v4(), "test".to_string(), &owner_key, &blobs)
        .await
        .unwrap();

    // Add content
    mount
        .add(
            &PathBuf::from("/file.txt"),
            Cursor::new(b"owner data".to_vec()),
        )
        .await
        .unwrap();

    // Add a mirror (unpublished)
    let mirror_key = SecretKey::generate();
    mount
        .add_principal(mirror_key.public(), PrincipalRole::Mirror)
        .await
        .unwrap();

    // Save
    let (link, _, _) = mount.save(&blobs).await.unwrap();

    // Owner can still mount even though there's an unpublished mirror
    let owner_mount = Mount::load(&link, &owner_key, &blobs)
        .await
        .expect("Owner should always be able to mount");

    let data = owner_mount.cat(&PathBuf::from("/file.txt")).await.unwrap();
    assert_eq!(data, b"owner data");
}
