//! Integration tests for Mount save/load persistence

mod common;

use ::common::mount::Mount;

#[tokio::test]
async fn test_save_load() {
    let (mount, blobs, secret_key, _temp) = common::setup_test_env().await;
    let (link, _previous_link, height) = mount.save(&blobs, false).await.unwrap();
    assert_eq!(height, 1); // Height should be 1 after first save
    let loaded_mount = Mount::load(&link, &secret_key, &blobs).await.unwrap();
    assert_eq!(loaded_mount.inner().await.height(), 1);
}
