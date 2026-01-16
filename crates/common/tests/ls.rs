//! Integration tests for Mount ls operations

mod common;

use std::io::Cursor;
use std::path::PathBuf;

#[tokio::test]
async fn test_ls() {
    let (mut mount, _, _, _temp) = common::setup_test_env().await;

    mount
        .add(&PathBuf::from("/file1.txt"), Cursor::new(b"data1".to_vec()))
        .await
        .unwrap();
    mount
        .add(&PathBuf::from("/file2.txt"), Cursor::new(b"data2".to_vec()))
        .await
        .unwrap();
    mount
        .add(
            &PathBuf::from("/dir/file3.txt"),
            Cursor::new(b"data3".to_vec()),
        )
        .await
        .unwrap();

    let items = mount.ls(&PathBuf::from("/")).await.unwrap();
    assert_eq!(items.len(), 3);

    assert!(items.contains_key(&PathBuf::from("file1.txt")));
    assert!(items.contains_key(&PathBuf::from("file2.txt")));
    assert!(items.contains_key(&PathBuf::from("dir")));

    let sub_items = mount.ls(&PathBuf::from("/dir")).await.unwrap();
    assert_eq!(sub_items.len(), 1);
    assert!(sub_items.contains_key(&PathBuf::from("dir/file3.txt")));
}

#[tokio::test]
async fn test_ls_deep() {
    let (mut mount, _, _, _temp) = common::setup_test_env().await;

    mount
        .add(&PathBuf::from("/a.txt"), Cursor::new(b"a".to_vec()))
        .await
        .unwrap();
    mount
        .add(&PathBuf::from("/dir1/b.txt"), Cursor::new(b"b".to_vec()))
        .await
        .unwrap();
    mount
        .add(
            &PathBuf::from("/dir1/dir2/c.txt"),
            Cursor::new(b"c".to_vec()),
        )
        .await
        .unwrap();
    mount
        .add(
            &PathBuf::from("/dir1/dir2/dir3/d.txt"),
            Cursor::new(b"d".to_vec()),
        )
        .await
        .unwrap();

    let all_items = mount.ls_deep(&PathBuf::from("/")).await.unwrap();

    assert!(all_items.contains_key(&PathBuf::from("a.txt")));
    assert!(all_items.contains_key(&PathBuf::from("dir1")));
    assert!(all_items.contains_key(&PathBuf::from("dir1/b.txt")));
    assert!(all_items.contains_key(&PathBuf::from("dir1/dir2")));
    assert!(all_items.contains_key(&PathBuf::from("dir1/dir2/c.txt")));
    assert!(all_items.contains_key(&PathBuf::from("dir1/dir2/dir3")));
    assert!(all_items.contains_key(&PathBuf::from("dir1/dir2/dir3/d.txt")));
}

#[tokio::test]
async fn test_error_cases() {
    let (mount, _, _, _temp) = common::setup_test_env().await;

    let result = mount.cat(&PathBuf::from("/does_not_exist.txt")).await;
    assert!(result.is_err());

    let result = mount.ls(&PathBuf::from("/does_not_exist")).await;
    assert!(result.is_err() || result.unwrap().is_empty());

    let (mut mount, _, _, _temp) = common::setup_test_env().await;
    mount
        .add(
            &PathBuf::from("/dir/file.txt"),
            Cursor::new(b"data".to_vec()),
        )
        .await
        .unwrap();

    let result = mount.cat(&PathBuf::from("/dir")).await;
    assert!(result.is_err());
}
