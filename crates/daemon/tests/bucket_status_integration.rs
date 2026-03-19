//! Integration tests for bucket status management
//!
//! These tests verify the bucket status CRUD operations and the
//! effective status logic (backward compatibility for pre-migration buckets).

use uuid::Uuid;

use jax_daemon::{BucketStatus, Database};

/// Create an in-memory test database
async fn setup_test_db() -> Database {
    let db_url = url::Url::parse("sqlite::memory:").unwrap();
    Database::connect(&db_url).await.unwrap()
}

#[tokio::test]
async fn alice_creates_bucket_and_status_is_set() {
    // Alice creates a bucket and explicitly sets it to active
    let db = setup_test_db().await;
    let alice_bucket = Uuid::new_v4();

    db.set_bucket_status(&alice_bucket, BucketStatus::Active, None)
        .await
        .unwrap();

    let status = db.get_bucket_status(&alice_bucket).await.unwrap();
    assert_eq!(status, Some(BucketStatus::Active));
}

#[tokio::test]
async fn bob_shares_bucket_starts_as_pending() {
    // Bob shares a bucket with us — it should start as pending
    let db = setup_test_db().await;
    let shared_bucket = Uuid::new_v4();
    let bob_node_id = "abcdef1234567890";

    db.set_bucket_status(&shared_bucket, BucketStatus::Pending, Some(bob_node_id))
        .await
        .unwrap();

    let status = db.get_bucket_status(&shared_bucket).await.unwrap();
    assert_eq!(status, Some(BucketStatus::Pending));
}

#[tokio::test]
async fn legacy_bucket_without_status_row_defaults_to_active() {
    // Pre-migration buckets have no status row — should be treated as active
    let db = setup_test_db().await;
    let legacy_bucket = Uuid::new_v4();

    // No set_bucket_status call — simulates a pre-migration bucket
    let status = db.get_bucket_status(&legacy_bucket).await.unwrap();
    assert_eq!(status, None);

    let effective = db
        .get_effective_bucket_status(&legacy_bucket)
        .await
        .unwrap();
    assert_eq!(effective, BucketStatus::Active);
}

#[tokio::test]
async fn approve_transitions_pending_to_active() {
    // Approving a pending bucket should set it to active
    let db = setup_test_db().await;
    let bucket = Uuid::new_v4();

    // Start as pending (shared by remote peer)
    db.set_bucket_status(&bucket, BucketStatus::Pending, Some("peer123"))
        .await
        .unwrap();
    assert_eq!(
        db.get_effective_bucket_status(&bucket).await.unwrap(),
        BucketStatus::Pending
    );

    // Approve
    db.set_bucket_status(&bucket, BucketStatus::Active, None)
        .await
        .unwrap();
    assert_eq!(
        db.get_effective_bucket_status(&bucket).await.unwrap(),
        BucketStatus::Active
    );
}

#[tokio::test]
async fn ignore_stops_sync() {
    // Ignoring a bucket should set it to ignored
    let db = setup_test_db().await;
    let bucket = Uuid::new_v4();

    // Start as active
    db.set_bucket_status(&bucket, BucketStatus::Active, None)
        .await
        .unwrap();

    // Ignore
    db.set_bucket_status(&bucket, BucketStatus::Ignored, None)
        .await
        .unwrap();
    assert_eq!(
        db.get_effective_bucket_status(&bucket).await.unwrap(),
        BucketStatus::Ignored
    );
}

#[tokio::test]
async fn upsert_overwrites_previous_status() {
    // Setting status twice should upsert, not create duplicate rows
    let db = setup_test_db().await;
    let bucket = Uuid::new_v4();

    db.set_bucket_status(&bucket, BucketStatus::Pending, Some("peer1"))
        .await
        .unwrap();
    db.set_bucket_status(&bucket, BucketStatus::Active, None)
        .await
        .unwrap();
    db.set_bucket_status(&bucket, BucketStatus::Ignored, None)
        .await
        .unwrap();

    // Should be the last status set
    assert_eq!(
        db.get_effective_bucket_status(&bucket).await.unwrap(),
        BucketStatus::Ignored
    );
}
