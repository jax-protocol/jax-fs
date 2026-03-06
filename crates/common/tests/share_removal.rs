//! Integration tests for share removal

mod common;

use ::common::crypto::SecretKey;
use ::common::mount::MountError;

#[tokio::test]
async fn test_owner_can_remove_share() {
    let (mut mount, blobs, _owner_key, _temp) = common::setup_test_env().await;

    // Add a peer as owner
    let peer_key = SecretKey::generate();
    mount.add_owner(peer_key.public()).await.unwrap();
    mount.save(&blobs).await.unwrap();

    // Verify the peer is in shares
    let inner = mount.inner().await;
    assert!(inner
        .manifest()
        .shares()
        .contains_key(&peer_key.public().to_hex()));
    drop(inner);

    // Remove the peer
    mount.remove_share(peer_key.public()).await.unwrap();

    // Verify removed
    let inner = mount.inner().await;
    assert!(!inner
        .manifest()
        .shares()
        .contains_key(&peer_key.public().to_hex()));
}

#[tokio::test]
async fn test_owner_can_remove_mirror() {
    let (mut mount, blobs, _owner_key, _temp) = common::setup_test_env().await;

    // Add a mirror
    let mirror_key = SecretKey::generate();
    mount.add_mirror(mirror_key.public()).await;
    mount.save(&blobs).await.unwrap();

    // Verify the mirror is in shares
    let inner = mount.inner().await;
    assert!(inner
        .manifest()
        .shares()
        .contains_key(&mirror_key.public().to_hex()));
    drop(inner);

    // Remove the mirror
    mount.remove_share(mirror_key.public()).await.unwrap();

    // Verify removed
    let inner = mount.inner().await;
    assert!(!inner
        .manifest()
        .shares()
        .contains_key(&mirror_key.public().to_hex()));
}

#[tokio::test]
async fn test_mirror_cannot_remove_shares() {
    let (mut mount, blobs, _owner_key, _temp) = common::setup_test_env().await;

    // Add a second owner and a mirror
    let peer_key = SecretKey::generate();
    let mirror_key = SecretKey::generate();
    mount.add_owner(peer_key.public()).await.unwrap();
    mount.add_mirror(mirror_key.public()).await;

    // Publish so the mirror can load
    let (link, _, _) = mount.publish().await.unwrap();

    // Load mount as mirror
    let mirror_mount = ::common::mount::Mount::load(&link, &mirror_key, &blobs)
        .await
        .expect("Mirror should be able to load published bucket");

    // Mirror tries to remove the other owner — should be rejected
    let result = mirror_mount.remove_share(peer_key.public()).await;
    assert!(
        matches!(result, Err(MountError::Unauthorized)),
        "Mirror should not be able to remove shares, got: {:?}",
        result
    );
}

#[tokio::test]
async fn test_remove_nonexistent_share_returns_error() {
    let (mount, _blobs, _owner_key, _temp) = common::setup_test_env().await;

    // Try to remove a peer that doesn't exist
    let nonexistent_key = SecretKey::generate();
    let result = mount.remove_share(nonexistent_key.public()).await;

    assert!(
        matches!(result, Err(MountError::ShareNotFound)),
        "Removing nonexistent share should return ShareNotFound, got: {:?}",
        result
    );
}

#[tokio::test]
async fn test_removed_peer_cannot_load_new_version() {
    let (mut mount, blobs, _owner_key, _temp) = common::setup_test_env().await;

    // Add a peer as owner
    let peer_key = SecretKey::generate();
    mount.add_owner(peer_key.public()).await.unwrap();
    let (link_before, _, _) = mount.save(&blobs).await.unwrap();

    // Verify peer can load the version before removal
    let _peer_mount = ::common::mount::Mount::load(&link_before, &peer_key, &blobs)
        .await
        .expect("Peer should be able to load mount before removal");

    // Remove the peer
    mount.remove_share(peer_key.public()).await.unwrap();
    let (link_after, _, _) = mount.save(&blobs).await.unwrap();

    // Peer should NOT be able to load the new version
    let result = ::common::mount::Mount::load(&link_after, &peer_key, &blobs).await;
    assert!(
        matches!(&result, Err(MountError::ShareNotFound)),
        "Removed peer should not be able to load mount after removal",
    );
}
