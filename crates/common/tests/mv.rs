//! Integration tests for Mount mv (move/rename) operations

mod common;

use std::io::Cursor;
use std::path::PathBuf;

use ::common::mount::MountError;

#[tokio::test]
async fn test_mv_file() {
    let (mut mount, _, _, _temp) = common::setup_test_env().await;

    // Create a file
    mount
        .add(&PathBuf::from("/old.txt"), Cursor::new(b"data".to_vec()))
        .await
        .unwrap();

    // Move the file
    mount
        .mv(&PathBuf::from("/old.txt"), &PathBuf::from("/new.txt"))
        .await
        .unwrap();

    // Verify old path doesn't exist
    let result = mount.cat(&PathBuf::from("/old.txt")).await;
    assert!(result.is_err());

    // Verify new path exists with same content
    let data = mount.cat(&PathBuf::from("/new.txt")).await.unwrap();
    assert_eq!(data, b"data");
}

#[tokio::test]
async fn test_mv_file_to_subdir() {
    let (mut mount, _, _, _temp) = common::setup_test_env().await;

    // Create a file
    mount
        .add(&PathBuf::from("/file.txt"), Cursor::new(b"data".to_vec()))
        .await
        .unwrap();

    // Move to a new subdirectory (should create it)
    mount
        .mv(
            &PathBuf::from("/file.txt"),
            &PathBuf::from("/subdir/file.txt"),
        )
        .await
        .unwrap();

    // Verify old path doesn't exist
    let result = mount.cat(&PathBuf::from("/file.txt")).await;
    assert!(result.is_err());

    // Verify new path exists
    let data = mount.cat(&PathBuf::from("/subdir/file.txt")).await.unwrap();
    assert_eq!(data, b"data");
}

#[tokio::test]
async fn test_mv_directory() {
    let (mut mount, _, _, _temp) = common::setup_test_env().await;

    // Create a directory with files
    mount
        .add(
            &PathBuf::from("/olddir/file1.txt"),
            Cursor::new(b"data1".to_vec()),
        )
        .await
        .unwrap();
    mount
        .add(
            &PathBuf::from("/olddir/file2.txt"),
            Cursor::new(b"data2".to_vec()),
        )
        .await
        .unwrap();

    // Move the directory
    mount
        .mv(&PathBuf::from("/olddir"), &PathBuf::from("/newdir"))
        .await
        .unwrap();

    // Verify old directory doesn't exist
    let result = mount.ls(&PathBuf::from("/olddir")).await;
    assert!(result.is_err());

    // Verify new directory exists with files
    let items = mount.ls(&PathBuf::from("/newdir")).await.unwrap();
    assert_eq!(items.len(), 2);

    // Verify file contents
    let data = mount
        .cat(&PathBuf::from("/newdir/file1.txt"))
        .await
        .unwrap();
    assert_eq!(data, b"data1");
}

#[tokio::test]
async fn test_mv_not_found() {
    let (mut mount, _, _, _temp) = common::setup_test_env().await;

    // Try to move a non-existent file
    let result = mount
        .mv(
            &PathBuf::from("/nonexistent.txt"),
            &PathBuf::from("/new.txt"),
        )
        .await;
    assert!(matches!(result, Err(MountError::PathNotFound(_))));
}

#[tokio::test]
async fn test_mv_already_exists() {
    let (mut mount, _, _, _temp) = common::setup_test_env().await;

    // Create two files
    mount
        .add(&PathBuf::from("/file1.txt"), Cursor::new(b"data1".to_vec()))
        .await
        .unwrap();
    mount
        .add(&PathBuf::from("/file2.txt"), Cursor::new(b"data2".to_vec()))
        .await
        .unwrap();

    // Try to move file1 to file2 (should fail)
    let result = mount
        .mv(&PathBuf::from("/file1.txt"), &PathBuf::from("/file2.txt"))
        .await;
    assert!(matches!(result, Err(MountError::PathAlreadyExists(_))));
}

#[tokio::test]
async fn test_mv_into_self() {
    let (mut mount, _, _, _temp) = common::setup_test_env().await;

    // Create a directory with a file inside
    mount.mkdir(&PathBuf::from("/parent")).await.unwrap();
    mount
        .add(
            &PathBuf::from("/parent/child.txt"),
            Cursor::new(b"data".to_vec()),
        )
        .await
        .unwrap();

    // Try to move directory into itself (should fail)
    let result = mount
        .mv(&PathBuf::from("/parent"), &PathBuf::from("/parent/nested"))
        .await;
    assert!(matches!(result, Err(MountError::MoveIntoSelf { .. })));

    // Try to move directory to same path (should also fail)
    let result = mount
        .mv(&PathBuf::from("/parent"), &PathBuf::from("/parent"))
        .await;
    assert!(matches!(result, Err(MountError::MoveIntoSelf { .. })));

    // Verify original directory still exists and is intact
    let items = mount.ls(&PathBuf::from("/parent")).await.unwrap();
    assert_eq!(items.len(), 1);

    // Verify child file still accessible
    let data = mount
        .cat(&PathBuf::from("/parent/child.txt"))
        .await
        .unwrap();
    assert_eq!(data, b"data");
}
