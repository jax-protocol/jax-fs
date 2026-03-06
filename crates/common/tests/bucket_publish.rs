//! Integration tests for bucket publish operations
//!
//! Tests cover owner publish, publish persistence, and explicit unpublish.

mod common;

use std::io::Cursor;
use std::path::PathBuf;

use ::common::crypto::SecretKey;
use ::common::mount::Mount;

const TEST_PATH: &str = "/file.txt";

#[tokio::test]
async fn test_owner_can_publish() {
    let (mut mount, blobs, _owner_key, _temp_dir) = common::setup_test_env().await;

    mount
        .add(&PathBuf::from(TEST_PATH), Cursor::new(b"hello".to_vec()))
        .await
        .unwrap();

    // Add a mirror so publish has an effect
    let mirror_key = SecretKey::generate();
    mount.add_mirror(mirror_key.public()).await;

    // Owner publishes
    let (link, _, _) = mount.publish().await.unwrap();

    // Verify manifest has plaintext secret (is_published)
    let published_mount = Mount::load(&link, &mirror_key, &blobs)
        .await
        .expect("Mirror should mount published bucket");
    assert!(published_mount.is_published().await);

    // Mirror can decrypt content
    let data = published_mount
        .cat(&PathBuf::from(TEST_PATH))
        .await
        .unwrap();
    assert_eq!(data, b"hello");
}

#[tokio::test]
async fn test_publish_persists_across_saves() {
    let (mut mount, blobs, _owner_key, _temp_dir) = common::setup_test_env().await;

    mount
        .add(&PathBuf::from(TEST_PATH), Cursor::new(b"secret".to_vec()))
        .await
        .unwrap();

    let mirror_key = SecretKey::generate();
    mount.add_mirror(mirror_key.public()).await;

    // Publish
    let (link_pub, _, _) = mount.publish().await.unwrap();
    let mirror_mount = Mount::load(&link_pub, &mirror_key, &blobs).await.unwrap();
    assert!(mirror_mount.is_published().await);

    // Save again without explicit publish - status should be preserved
    mount
        .add(
            &PathBuf::from("/another.txt"),
            Cursor::new(b"more data".to_vec()),
        )
        .await
        .unwrap();
    let (link_saved, _, _) = mount.save(&blobs).await.unwrap();

    // Mirror should still be able to mount
    let mirror_mount2 = Mount::load(&link_saved, &mirror_key, &blobs)
        .await
        .expect("Mirror should still mount after save() on published bucket");
    assert!(mirror_mount2.is_published().await);
}

#[tokio::test]
async fn test_publish_then_unpublish_round_trip() {
    let (mut mount, blobs, owner_key, _temp_dir) = common::setup_test_env().await;

    mount
        .add(&PathBuf::from(TEST_PATH), Cursor::new(b"secret".to_vec()))
        .await
        .unwrap();

    let mirror_key = SecretKey::generate();
    mount.add_mirror(mirror_key.public()).await;

    // Publish
    let (link_pub, _, _) = mount.publish().await.unwrap();
    let mirror_mount = Mount::load(&link_pub, &mirror_key, &blobs).await.unwrap();
    assert!(mirror_mount.is_published().await);

    // Explicitly unpublish
    let owner_mount = Mount::load(&link_pub, &owner_key, &blobs).await.unwrap();
    let (link_unpub, _, _) = owner_mount.unpublish().await.unwrap();

    // Mirror should no longer be able to mount
    let result = Mount::load(&link_unpub, &mirror_key, &blobs).await;
    assert!(result.is_err(), "Mirror should not mount after unpublish");

    // Owner can still mount
    let owner_mount2 = Mount::load(&link_unpub, &owner_key, &blobs).await.unwrap();
    assert!(!owner_mount2.is_published().await);
}
