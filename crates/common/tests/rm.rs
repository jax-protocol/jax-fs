//! Integration tests for Mount rm (remove) operations

mod common;

use std::io::Cursor;
use std::path::PathBuf;

#[tokio::test]
async fn test_rm() {
    let (mut mount, _, _, _temp) = common::setup_test_env().await;

    mount
        .add(&PathBuf::from("/file1.txt"), Cursor::new(b"data1".to_vec()))
        .await
        .unwrap();
    mount
        .add(&PathBuf::from("/file2.txt"), Cursor::new(b"data2".to_vec()))
        .await
        .unwrap();

    let items = mount.ls(&PathBuf::from("/")).await.unwrap();
    assert_eq!(items.len(), 2);

    mount.rm(&PathBuf::from("/file1.txt")).await.unwrap();

    let items = mount.ls(&PathBuf::from("/")).await.unwrap();
    assert_eq!(items.len(), 1);
    assert!(items.contains_key(&PathBuf::from("file2.txt")));
    assert!(!items.contains_key(&PathBuf::from("file1.txt")));

    let result = mount.cat(&PathBuf::from("/file1.txt")).await;
    assert!(result.is_err());
}
