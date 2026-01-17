//! Integration tests for Mount operations log (CRDT)

mod common;

use std::io::Cursor;
use std::path::PathBuf;

use ::common::mount::{Mount, OpType};

#[tokio::test]
async fn test_ops_log_records_operations() {
    let (mut mount, _, _, _temp) = common::setup_test_env().await;

    // Perform various operations
    mount
        .add(&PathBuf::from("/file1.txt"), Cursor::new(b"data".to_vec()))
        .await
        .unwrap();
    mount.mkdir(&PathBuf::from("/dir")).await.unwrap();
    mount
        .mv(
            &PathBuf::from("/file1.txt"),
            &PathBuf::from("/dir/file1.txt"),
        )
        .await
        .unwrap();
    mount.rm(&PathBuf::from("/dir/file1.txt")).await.unwrap();

    // Verify ops log has recorded all operations
    let inner = mount.inner().await;
    let ops_log = inner.ops_log();

    // Should have 4 operations: Add, Mkdir, Mv, Remove
    assert_eq!(ops_log.len(), 4);

    let ops: Vec<_> = ops_log.ops_in_order().collect();
    assert!(matches!(ops[0].op_type, OpType::Add));
    assert!(matches!(ops[1].op_type, OpType::Mkdir));
    assert!(matches!(ops[2].op_type, OpType::Mv { .. }));
    assert!(matches!(ops[3].op_type, OpType::Remove));
}

#[tokio::test]
async fn test_ops_log_persists_across_save_load() {
    let (mut mount, blobs, secret_key, _temp) = common::setup_test_env().await;

    // Perform some operations
    mount
        .add(&PathBuf::from("/file.txt"), Cursor::new(b"data".to_vec()))
        .await
        .unwrap();
    mount.mkdir(&PathBuf::from("/dir")).await.unwrap();

    // Save
    let (link, _, _) = mount.save(&blobs, false).await.unwrap();

    // Load the mount
    let loaded_mount = Mount::load(&link, &secret_key, &blobs).await.unwrap();

    // Verify ops log was loaded
    let inner = loaded_mount.inner().await;
    let ops_log = inner.ops_log();
    assert_eq!(ops_log.len(), 2);

    let ops: Vec<_> = ops_log.ops_in_order().collect();
    assert!(matches!(ops[0].op_type, OpType::Add));
    assert!(matches!(ops[1].op_type, OpType::Mkdir));
}
