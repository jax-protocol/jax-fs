/// Lightweight test harness for multi-peer integration tests
///
/// This module provides a simple way to create and test multiple peers
/// communicating with each other in-process, without requiring external
/// infrastructure.
///
/// # Example
///
/// ```rust,ignore
/// use common::testkit::TestNetwork;
///
/// #[tokio::test]
/// async fn test_peer_sync() -> anyhow::Result<()> {
///     let mut net = TestNetwork::new();
///
///     // Create two test peers
///     let alice = net.add_peer("alice").await?;
///     let bob = net.add_peer("bob").await?;
///
///     // Alice creates and commits to a bucket
///     let bucket_id = alice.create_bucket("shared-bucket").await?;
///     alice.commit_bucket(bucket_id, "manifest data").await?;
///
///     // Bob can sync from Alice
///     bob.sync_from(&alice, bucket_id).await?;
///
///     // Cleanup
///     net.shutdown().await?;
///     Ok(())
/// }
/// ```
mod network;
mod peer;

pub use network::TestNetwork;
pub use peer::TestPeer;
