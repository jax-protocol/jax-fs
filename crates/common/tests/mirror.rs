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
    let (link, _, _) = mount.save(&blobs).await.unwrap();

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
    let (link, _, _) = mount.save(&blobs).await.unwrap();

    // Owner can mount regardless of publish state
    let owner_mount = Mount::load(&link, &owner_key, &blobs)
        .await
        .expect("Owner should always be able to mount");

    let data = owner_mount.cat(&PathBuf::from(TEST_PATH)).await.unwrap();
    assert_eq!(data, b"owner data");
}

#[tokio::test]
async fn test_save_preserves_publish_status() {
    // This tests that save() preserves publish state,
    // so a published bucket stays published after subsequent saves
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
    let (link2, _, _) = mount.save(&blobs).await.unwrap();

    // Mirror SHOULD still be able to mount because publish status is preserved
    let mirror_mount2 = Mount::load(&link2, &mirror_key, &blobs)
        .await
        .expect("Mirror should mount after save() on published bucket");
    assert!(mirror_mount2.is_published().await);

    // Mirror can read the new file too
    let data = mirror_mount2
        .cat(&PathBuf::from("/new_file.txt"))
        .await
        .unwrap();
    assert_eq!(data, b"new data");
}

#[tokio::test]
async fn test_unpublish_clears_public_secret() {
    // This tests that unpublish() clears the public secret,
    // making the bucket private again (mirrors can no longer mount)
    let (mount, blobs, owner_key, mirror_key, _temp) =
        setup_mount_with_mirror(b"initial data").await;

    // First publish
    let (link1, _, _) = mount.publish().await.unwrap();

    // Verify mirror can mount the published version
    let mirror_mount = Mount::load(&link1, &mirror_key, &blobs)
        .await
        .expect("Mirror should be able to mount published bucket");
    assert!(mirror_mount.is_published().await);

    // Explicitly unpublish
    let owner_mount = Mount::load(&link1, &owner_key, &blobs).await.unwrap();
    let (link2, _, _) = owner_mount.unpublish().await.unwrap();

    // Mirror should NOT be able to mount because we unpublished
    let result = Mount::load(&link2, &mirror_key, &blobs).await;
    assert!(
        matches!(&result, Err(MountError::MirrorCannotMount)),
        "Mirror should not mount after unpublish: {:?}",
        result.err()
    );

    // Owner can still mount
    let owner_mount2 = Mount::load(&link2, &owner_key, &blobs).await.unwrap();
    assert!(!owner_mount2.is_published().await);
}

#[tokio::test]
async fn test_mirror_can_mount_after_mv_on_published_bucket() {
    // This tests that mirrors can still mount after mv on a published bucket,
    // because publish status is preserved across saves
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

    // Do a mv operation and save WITHOUT publish flag
    mount
        .mv(&PathBuf::from(TEST_PATH), &PathBuf::from("/docs/moved.txt"))
        .await
        .unwrap();
    let (link2, _, _) = mount.save(&blobs).await.unwrap();

    // Mirror should STILL be able to mount because publish status is preserved
    let mirror_mount2 = Mount::load(&link2, &mirror_key, &blobs)
        .await
        .expect("Mirror should mount after mv on published bucket");
    assert!(
        mirror_mount2.is_published().await,
        "Bucket should remain published after save()"
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
    // 2. After mv with save(), mirror CAN still mount HEAD (publish persists)
    // 3. After explicit unpublish, mirror can no longer mount

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
    let (_, _, _) = mount.save(&blobs).await.unwrap();

    // Share with mirror (as owner)
    let mirror_owner_key = SecretKey::generate();
    mount.add_owner(mirror_owner_key.public()).await.unwrap();
    let (_, _, _) = mount.save(&blobs).await.unwrap();

    // Share with mirror
    let mirror_key = SecretKey::generate();
    mount.add_mirror(mirror_key.public()).await;
    let (_, _, _) = mount.save(&blobs).await.unwrap();

    // Publish - this makes the bucket publicly accessible to mirrors
    let (link_published, _, _) = mount.publish().await.unwrap();

    // Verify mirror can mount after publish
    let mirror_mount = Mount::load(&link_published, &mirror_key, &blobs)
        .await
        .expect("Mirror should mount published bucket");
    assert!(mirror_mount.is_published().await);

    // mv operation with save() - publish status is PRESERVED
    mount
        .mv(
            &PathBuf::from("/hello.txt"),
            &PathBuf::from("/docs/hello.txt"),
        )
        .await
        .unwrap();
    let (link_after_mv, _, _) = mount.save(&blobs).await.unwrap();

    // Mirror CAN mount HEAD because publish status is preserved
    let mirror_head = Mount::load(&link_after_mv, &mirror_key, &blobs)
        .await
        .expect("Mirror should mount HEAD since publish is preserved");
    assert!(mirror_head.is_published().await);

    // Mirror sees the moved file at the new location
    let data = mirror_head
        .cat(&PathBuf::from("/docs/hello.txt"))
        .await
        .unwrap();
    assert_eq!(data, b"hello world");

    // Owner can mount HEAD
    let owner_mount = Mount::load(&link_after_mv, &owner_key, &blobs)
        .await
        .expect("Owner should always mount");
    assert!(owner_mount.is_published().await, "HEAD should be published");

    // Explicit unpublish makes bucket private again
    let (link_unpublished, _, _) = owner_mount.unpublish().await.unwrap();

    let result = Mount::load(&link_unpublished, &mirror_key, &blobs).await;
    assert!(
        matches!(&result, Err(MountError::MirrorCannotMount)),
        "Mirror should NOT mount after unpublish: {:?}",
        result.err()
    );
}
