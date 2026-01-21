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

#[tokio::test]
async fn test_save_without_publish_clears_public_secret() {
    // This tests that save(publish=false) clears the public secret,
    // making the bucket private again (mirrors can no longer mount)
    let (mut mount, blobs, _, mirror_key, _temp) = setup_mount_with_mirror(b"initial data").await;

    // First publish
    let (link1, _, _) = mount.publish().await.unwrap();

    // Verify mirror can mount the published version
    let mirror_mount = Mount::load(&link1, &mirror_key, &blobs)
        .await
        .expect("Mirror should be able to mount published bucket");
    assert!(mirror_mount.is_published().await);

    // Now add more content and save WITHOUT the publish flag
    mount
        .add(
            &PathBuf::from("/new_file.txt"),
            Cursor::new(b"new data".to_vec()),
        )
        .await
        .unwrap();
    let (link2, _, _) = mount.save(&blobs, false).await.unwrap();

    // Mirror should NOT be able to mount because save(false) clears the public secret
    let result = Mount::load(&link2, &mirror_key, &blobs).await;
    assert!(
        matches!(&result, Err(MountError::MirrorCannotMount)),
        "Mirror should not be able to mount after save(publish=false): {:?}",
        result.err()
    );
}

#[tokio::test]
async fn test_mirror_can_mount_after_mv_with_republish() {
    // This tests that mirrors can mount after mv IF we republish
    let (mut mount, blobs, _, mirror_key, _temp) = setup_mount_with_mirror(b"file content").await;

    // Add another file for the mv test
    mount
        .add(
            &PathBuf::from("/docs/readme.md"),
            Cursor::new(b"readme".to_vec()),
        )
        .await
        .unwrap();

    // Publish
    let (link1, _, _) = mount.publish().await.unwrap();

    // Verify mirror can mount
    let mirror_mount = Mount::load(&link1, &mirror_key, &blobs)
        .await
        .expect("Mirror should be able to mount published bucket");
    assert!(mirror_mount.is_published().await);

    // Now do a mv operation and REPUBLISH to maintain mirror access
    mount
        .mv(&PathBuf::from(TEST_PATH), &PathBuf::from("/docs/moved.txt"))
        .await
        .unwrap();
    let (link2, _, _) = mount.save(&blobs, true).await.unwrap(); // publish=true!

    // Mirror should be able to mount because we republished
    let result = Mount::load(&link2, &mirror_key, &blobs).await;
    assert!(
        result.is_ok(),
        "Mirror should mount after mv with republish: {:?}",
        result.err()
    );

    let mirror_mount2 = result.unwrap();
    assert!(
        mirror_mount2.is_published().await,
        "Bucket should be published after save(true)"
    );

    // Verify the mv happened
    let data = mirror_mount2
        .cat(&PathBuf::from("/docs/moved.txt"))
        .await
        .unwrap();
    assert_eq!(data, b"file content");
}

#[tokio::test]
async fn test_full_fixture_flow() {
    // Simulates the fixture flow and verifies:
    // 1. After publish, mirror can mount
    // 2. After mv with save(false), mirror CANNOT mount HEAD (it's private)
    // 3. But mirror CAN still mount the previously published version
    // (Gateway should show the last published version, not HEAD)

    let (mut mount, blobs, owner_key, _temp_dir) = common::setup_test_env().await;

    // Add files
    mount
        .add(
            &PathBuf::from("/hello.txt"),
            Cursor::new(b"hello world".to_vec()),
        )
        .await
        .unwrap();
    mount
        .add(
            &PathBuf::from("/docs/readme.md"),
            Cursor::new(b"readme content".to_vec()),
        )
        .await
        .unwrap();

    // Save after adding files
    let (_, _, _) = mount.save(&blobs, false).await.unwrap();

    // Share with mirror (as owner)
    let mirror_owner_key = SecretKey::generate();
    mount.add_owner(mirror_owner_key.public()).await.unwrap();
    let (_, _, _) = mount.save(&blobs, false).await.unwrap();

    // Share with mirror
    let mirror_key = SecretKey::generate();
    mount.add_mirror(mirror_key.public()).await;
    let (_, _, _) = mount.save(&blobs, false).await.unwrap();

    // Publish - this is the "last published version"
    let (link_published, _, _) = mount.publish().await.unwrap();

    // Verify mirror can mount after publish
    let mirror_mount = Mount::load(&link_published, &mirror_key, &blobs)
        .await
        .expect("Mirror should mount published bucket");
    assert!(mirror_mount.is_published().await);

    // mv operation with save(false) - bucket becomes private at HEAD
    mount
        .mv(
            &PathBuf::from("/hello.txt"),
            &PathBuf::from("/docs/hello.txt"),
        )
        .await
        .unwrap();
    let (link_after_mv, _, _) = mount.save(&blobs, false).await.unwrap();

    // Mirror CANNOT mount HEAD because it's now private
    let result = Mount::load(&link_after_mv, &mirror_key, &blobs).await;
    assert!(
        matches!(&result, Err(MountError::MirrorCannotMount)),
        "Mirror should NOT mount HEAD after save(false): {:?}",
        result.err()
    );

    // But mirror CAN still mount the previously published version
    let mirror_mount_old = Mount::load(&link_published, &mirror_key, &blobs)
        .await
        .expect("Mirror should still mount the last published version");
    assert!(mirror_mount_old.is_published().await);

    // The published version has the OLD content (before mv)
    let data = mirror_mount_old
        .cat(&PathBuf::from("/hello.txt"))
        .await
        .unwrap();
    assert_eq!(data, b"hello world");

    // Owner can still mount HEAD (private version with moved file)
    let owner_mount = Mount::load(&link_after_mv, &owner_key, &blobs)
        .await
        .expect("Owner should always mount");
    assert!(!owner_mount.is_published().await, "HEAD should be private");

    // Owner sees the moved file at the new location
    let data = owner_mount
        .cat(&PathBuf::from("/docs/hello.txt"))
        .await
        .unwrap();
    assert_eq!(data, b"hello world");
}
