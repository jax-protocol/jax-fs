//! Integration tests for the bucket ACL event log.
//!
//! These tests verify the append-only ACL event log, effective status
//! reduction (last event wins), and backward compatibility for legacy buckets.

use uuid::Uuid;

use jax_daemon::{BucketAclEvent, Database};

/// Create an isolated in-memory test database with all migrations applied.
async fn setup_test_db() -> Database {
    Database::memory().await.unwrap()
}

#[tokio::test]
async fn alice_creates_bucket_status_is_active() {
    // Alice creates a bucket locally — she appends an Approved event
    let db = setup_test_db().await;
    let alice_bucket = Uuid::new_v4();

    db.append_acl_event(&alice_bucket, BucketAclEvent::Approved, "self")
        .await
        .unwrap();

    let status = db.get_effective_acl_status(&alice_bucket).await.unwrap();
    assert_eq!(
        status.unwrap().as_str(),
        "active",
        "approved event should yield active status"
    );
}

#[tokio::test]
async fn bob_shares_bucket_starts_as_pending() {
    // Bob shares a bucket with us — a Shared event is recorded
    let db = setup_test_db().await;
    let shared_bucket = Uuid::new_v4();
    let bob_peer_id = "abcdef1234567890";

    db.append_acl_event(&shared_bucket, BucketAclEvent::Shared, bob_peer_id)
        .await
        .unwrap();

    let status = db.get_effective_acl_status(&shared_bucket).await.unwrap();
    assert_eq!(
        status.unwrap().as_str(),
        "pending",
        "shared event should yield pending status"
    );
}

#[tokio::test]
async fn approve_transitions_pending_to_active() {
    // Approving a pending bucket transitions it to active
    let db = setup_test_db().await;
    let bucket = Uuid::new_v4();

    // Shared by remote peer
    db.append_acl_event(&bucket, BucketAclEvent::Shared, "peer123")
        .await
        .unwrap();
    assert_eq!(
        db.get_effective_acl_status(&bucket)
            .await
            .unwrap()
            .unwrap()
            .as_str(),
        "pending"
    );

    // User approves
    db.append_acl_event(&bucket, BucketAclEvent::Approved, "self")
        .await
        .unwrap();
    assert_eq!(
        db.get_effective_acl_status(&bucket)
            .await
            .unwrap()
            .unwrap()
            .as_str(),
        "active"
    );
}

#[tokio::test]
async fn ignore_marks_bucket_terminal() {
    // Ignoring a bucket makes it terminal
    let db = setup_test_db().await;
    let bucket = Uuid::new_v4();

    db.append_acl_event(&bucket, BucketAclEvent::Shared, "peer1")
        .await
        .unwrap();
    db.append_acl_event(&bucket, BucketAclEvent::Ignored, "self")
        .await
        .unwrap();

    let status = db.get_effective_acl_status(&bucket).await.unwrap().unwrap();
    assert_eq!(status.as_str(), "ignored");
    assert!(status.is_terminal());
}

#[tokio::test]
async fn leave_marks_bucket_terminal() {
    // Leaving a bucket makes it terminal
    let db = setup_test_db().await;
    let bucket = Uuid::new_v4();

    db.append_acl_event(&bucket, BucketAclEvent::Approved, "self")
        .await
        .unwrap();
    db.append_acl_event(&bucket, BucketAclEvent::Left, "self")
        .await
        .unwrap();

    let status = db.get_effective_acl_status(&bucket).await.unwrap().unwrap();
    assert_eq!(status.as_str(), "left");
    assert!(status.is_terminal());
}

#[tokio::test]
async fn kicked_marks_bucket_terminal() {
    // Being kicked from a bucket makes it terminal
    let db = setup_test_db().await;
    let bucket = Uuid::new_v4();

    db.append_acl_event(&bucket, BucketAclEvent::Approved, "self")
        .await
        .unwrap();
    db.append_acl_event(&bucket, BucketAclEvent::Kicked, "peer_owner")
        .await
        .unwrap();

    let status = db.get_effective_acl_status(&bucket).await.unwrap().unwrap();
    assert_eq!(status.as_str(), "kicked");
    assert!(status.is_terminal());
}

#[tokio::test]
async fn last_event_wins() {
    // Multiple events — the last one determines effective status
    let db = setup_test_db().await;
    let bucket = Uuid::new_v4();

    db.append_acl_event(&bucket, BucketAclEvent::Shared, "peer1")
        .await
        .unwrap();
    db.append_acl_event(&bucket, BucketAclEvent::Approved, "self")
        .await
        .unwrap();
    db.append_acl_event(&bucket, BucketAclEvent::Ignored, "self")
        .await
        .unwrap();

    // Ignored is the last event
    assert_eq!(
        db.get_effective_acl_status(&bucket)
            .await
            .unwrap()
            .unwrap()
            .as_str(),
        "ignored"
    );
}

#[tokio::test]
async fn unknown_bucket_returns_none() {
    // A bucket with no ACL events and no bucket_log entries returns None
    let db = setup_test_db().await;
    let unknown = Uuid::new_v4();

    let status = db.get_effective_acl_status(&unknown).await.unwrap();
    assert!(status.is_none());
}

#[tokio::test]
async fn audit_trail_preserves_all_events() {
    // The full event log is preserved for audit purposes
    let db = setup_test_db().await;
    let bucket = Uuid::new_v4();

    db.append_acl_event(&bucket, BucketAclEvent::Shared, "peer1")
        .await
        .unwrap();
    db.append_acl_event(&bucket, BucketAclEvent::Approved, "self")
        .await
        .unwrap();
    db.append_acl_event(&bucket, BucketAclEvent::Kicked, "peer_owner")
        .await
        .unwrap();

    let events = db.list_acl_events(&bucket).await.unwrap();
    assert_eq!(events.len(), 3);

    assert_eq!(events[0].0, BucketAclEvent::Shared);
    assert_eq!(events[0].1, "peer1");

    assert_eq!(events[1].0, BucketAclEvent::Approved);
    assert_eq!(events[1].1, "self");

    assert_eq!(events[2].0, BucketAclEvent::Kicked);
    assert_eq!(events[2].1, "peer_owner");
}
