//! Integration tests for Mount mkdir operations

mod common;

use std::io::Cursor;
use std::path::PathBuf;

use ::common::mount::MountError;

#[tokio::test]
async fn test_mkdir() {
    let (mut mount, _, _, _temp) = common::setup_test_env().await;

    // Create a directory
    mount.mkdir(&PathBuf::from("/test_dir")).await.unwrap();

    // Verify it exists and is a directory
    let items = mount.ls(&PathBuf::from("/")).await.unwrap();
    assert_eq!(items.len(), 1);
    assert!(items.get(&PathBuf::from("test_dir")).unwrap().is_dir());
}

#[tokio::test]
async fn test_mkdir_nested() {
    let (mut mount, _, _, _temp) = common::setup_test_env().await;

    // Create nested directories (should create parents automatically)
    mount.mkdir(&PathBuf::from("/a/b/c")).await.unwrap();

    // Verify the whole path exists
    let items_root = mount.ls(&PathBuf::from("/")).await.unwrap();
    assert!(items_root.contains_key(&PathBuf::from("a")));

    let items_a = mount.ls(&PathBuf::from("/a")).await.unwrap();
    assert!(items_a.contains_key(&PathBuf::from("a/b")));

    let items_b = mount.ls(&PathBuf::from("/a/b")).await.unwrap();
    assert!(items_b.contains_key(&PathBuf::from("a/b/c")));
    assert!(items_b.get(&PathBuf::from("a/b/c")).unwrap().is_dir());
}

#[tokio::test]
async fn test_mkdir_already_exists() {
    let (mut mount, _, _, _temp) = common::setup_test_env().await;

    // Create directory
    mount.mkdir(&PathBuf::from("/test_dir")).await.unwrap();

    // Try to create it again - should error
    let result = mount.mkdir(&PathBuf::from("/test_dir")).await;
    assert!(matches!(result, Err(MountError::PathAlreadyExists(_))));
}

#[tokio::test]
async fn test_mkdir_file_exists() {
    let (mut mount, _, _, _temp) = common::setup_test_env().await;

    // Create a file
    mount
        .add(&PathBuf::from("/test.txt"), Cursor::new(b"data".to_vec()))
        .await
        .unwrap();

    // Try to create directory with same name - should error
    let result = mount.mkdir(&PathBuf::from("/test.txt")).await;
    assert!(matches!(result, Err(MountError::PathAlreadyExists(_))));
}

#[tokio::test]
async fn test_mkdir_then_add_file() {
    let (mut mount, _, _, _temp) = common::setup_test_env().await;

    // Create a directory
    mount.mkdir(&PathBuf::from("/docs")).await.unwrap();

    // Add a file to the created directory
    mount
        .add(
            &PathBuf::from("/docs/readme.md"),
            Cursor::new(b"# README".to_vec()),
        )
        .await
        .unwrap();

    // Verify the file exists
    let data = mount.cat(&PathBuf::from("/docs/readme.md")).await.unwrap();
    assert_eq!(data, b"# README");

    // Verify directory structure
    let items = mount.ls(&PathBuf::from("/docs")).await.unwrap();
    assert_eq!(items.len(), 1);
    assert!(items.contains_key(&PathBuf::from("docs/readme.md")));
}

#[tokio::test]
async fn test_mkdir_multiple_siblings() {
    let (mut mount, _, _, _temp) = common::setup_test_env().await;

    // Create multiple sibling directories
    mount.mkdir(&PathBuf::from("/dir1")).await.unwrap();
    mount.mkdir(&PathBuf::from("/dir2")).await.unwrap();
    mount.mkdir(&PathBuf::from("/dir3")).await.unwrap();

    // Verify all exist
    let items = mount.ls(&PathBuf::from("/")).await.unwrap();
    assert_eq!(items.len(), 3);
    assert!(items.get(&PathBuf::from("dir1")).unwrap().is_dir());
    assert!(items.get(&PathBuf::from("dir2")).unwrap().is_dir());
    assert!(items.get(&PathBuf::from("dir3")).unwrap().is_dir());
}
