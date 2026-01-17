//! Integration tests for Mount add and cat operations

mod common;

use std::io::Cursor;
use std::path::PathBuf;

#[tokio::test]
async fn test_add_and_cat() {
    let (mut mount, _, _, _temp) = common::setup_test_env().await;

    let data = b"Hello, world!";
    let path = PathBuf::from("/test.txt");

    mount.add(&path, Cursor::new(data.to_vec())).await.unwrap();

    let result = mount.cat(&path).await.unwrap();
    assert_eq!(result, data);
}

#[tokio::test]
async fn test_add_with_metadata() {
    let (mut mount, _, _, _temp) = common::setup_test_env().await;

    let data = b"{ \"key\": \"value\" }";
    let path = PathBuf::from("/data.json");

    mount.add(&path, Cursor::new(data.to_vec())).await.unwrap();

    let items = mount.ls(&PathBuf::from("/")).await.unwrap();
    assert_eq!(items.len(), 1);

    let (file_path, link) = items.iter().next().unwrap();
    assert_eq!(file_path, &PathBuf::from("data.json"));

    if let Some(data_info) = link.data() {
        assert!(data_info.mime().is_some());
        assert_eq!(data_info.mime().unwrap().as_ref(), "application/json");
    } else {
        panic!("Expected data link with metadata");
    }
}

#[tokio::test]
async fn test_various_file_types() {
    let (mut mount, _, _, _temp) = common::setup_test_env().await;

    let test_files = vec![
        ("/image.png", "image/png"),
        ("/video.mp4", "video/mp4"),
        ("/style.css", "text/css"),
        ("/script.js", "text/javascript"),
        ("/data.json", "application/json"),
        ("/archive.zip", "application/zip"),
        ("/document.pdf", "application/pdf"),
        ("/code.rs", "text/x-rust"),
    ];

    for (path, expected_mime) in test_files {
        mount
            .add(&PathBuf::from(path), Cursor::new(b"test".to_vec()))
            .await
            .unwrap();

        let items = mount.ls(&PathBuf::from("/")).await.unwrap();
        let link = items.values().find(|l| l.is_data()).unwrap();

        if let Some(data_info) = link.data() {
            assert!(data_info.mime().is_some());
            assert_eq!(data_info.mime().unwrap().as_ref(), expected_mime);
        }

        mount.rm(&PathBuf::from(path)).await.unwrap();
    }
}

#[tokio::test]
async fn test_nested_operations() {
    let (mut mount, _, _, _temp) = common::setup_test_env().await;

    let files = vec![
        ("/root.txt", b"root" as &[u8]),
        ("/docs/readme.md", b"readme" as &[u8]),
        ("/docs/guide.pdf", b"guide" as &[u8]),
        ("/src/main.rs", b"main" as &[u8]),
        ("/src/lib.rs", b"lib" as &[u8]),
        ("/src/tests/unit.rs", b"unit" as &[u8]),
        ("/src/tests/integration.rs", b"integration" as &[u8]),
    ];

    for (path, data) in &files {
        mount
            .add(&PathBuf::from(path), Cursor::new(data.to_vec()))
            .await
            .unwrap();
    }

    for (path, expected_data) in &files {
        let data = mount.cat(&PathBuf::from(path)).await.unwrap();
        assert_eq!(data, expected_data.to_vec());
    }

    mount
        .rm(&PathBuf::from("/src/tests/unit.rs"))
        .await
        .unwrap();

    let result = mount.cat(&PathBuf::from("/src/tests/unit.rs")).await;
    assert!(result.is_err());

    let data = mount
        .cat(&PathBuf::from("/src/tests/integration.rs"))
        .await
        .unwrap();
    assert_eq!(data, b"integration");
}
