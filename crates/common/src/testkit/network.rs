use super::peer::TestPeer;
use anyhow::Result;
use std::collections::HashMap;
use std::time::Duration;

/// A coordinator for multiple test peers
///
/// TestNetwork manages the lifecycle of multiple peers and provides
/// utilities for eventual consistency testing.
pub struct TestNetwork {
    /// All peers in the network, indexed by name
    peers: HashMap<String, TestPeer>,
}

impl TestNetwork {
    /// Create a new test network
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
        }
    }

    /// Add a new peer to the network and start it
    ///
    /// # Arguments
    /// * `name` - Unique name for this peer
    pub async fn add_peer(&mut self, name: impl Into<String>) -> Result<()> {
        let name = name.into();

        if self.peers.contains_key(&name) {
            return Err(anyhow::anyhow!("Peer '{}' already exists", name));
        }

        let mut peer = TestPeer::new(name.clone(), None, None).await?;
        peer.start().await?;

        self.peers.insert(name.clone(), peer);

        Ok(())
    }

    /// Get a peer by name
    pub fn peer(&self, name: &str) -> Option<&TestPeer> {
        self.peers.get(name)
    }

    /// Get a mutable peer by name
    pub fn peer_mut(&mut self, name: &str) -> Option<&mut TestPeer> {
        self.peers.get_mut(name)
    }

    /// Get all peer names
    pub fn peer_names(&self) -> Vec<String> {
        self.peers.keys().cloned().collect()
    }

    /// Introduce all peers to each other for local discovery
    ///
    /// This manually adds each peer's direct socket addresses to all other peers,
    /// enabling immediate local connections without waiting for DHT propagation.
    pub fn introduce_all_peers(&mut self) -> Result<()> {
        // First, collect all peer info to avoid borrow checker issues
        let peer_info: Vec<_> = self
            .peers
            .iter()
            .map(|(name, peer)| {
                (
                    name.clone(),
                    peer.id(),
                    peer.peer()
                        .endpoint()
                        .bound_sockets()
                        .into_iter()
                        .collect::<Vec<_>>(),
                )
            })
            .collect();

        tracing::debug!("Introducing {} peers to each other", peer_info.len());

        // Now introduce each pair
        for i in 0..peer_info.len() {
            for j in 0..peer_info.len() {
                if i != j {
                    let (ref peer_a_name, _, _) = peer_info[i];
                    let (ref peer_b_name, peer_b_id, ref peer_b_addrs) = peer_info[j];

                    // Create NodeAddr for peer B
                    let node_addr = iroh::NodeAddr::from_parts(
                        peer_b_id,
                        None, // no relay needed for local
                        peer_b_addrs.clone(),
                    );

                    // Tell peer A about peer B
                    let peer_a = self.peers.get_mut(peer_a_name).unwrap();
                    peer_a
                        .peer()
                        .endpoint()
                        .add_node_addr_with_source(node_addr, "testkit")?;

                    tracing::trace!(
                        "Introduced {} to {} at {:?}",
                        peer_a_name,
                        peer_b_name,
                        peer_b_addrs
                    );
                }
            }
        }

        tracing::info!("All peers introduced to each other");
        Ok(())
    }

    /// Remove a peer from the network and stop it
    pub async fn remove_peer(&mut self, name: &str) -> Result<()> {
        if let Some(mut peer) = self.peers.remove(name) {
            peer.stop().await?;
        }
        Ok(())
    }

    /// Shutdown all peers in the network
    pub async fn shutdown(&mut self) -> Result<()> {
        tracing::info!("Shutting down test network with {} peers", self.peers.len());

        for (name, peer) in self.peers.iter_mut() {
            tracing::debug!("Stopping peer: {}", name);
            if let Err(e) = peer.stop().await {
                tracing::error!("Error stopping peer {}: {}", name, e);
            }
        }

        self.peers.clear();
        tracing::info!("Test network shut down complete");

        Ok(())
    }

    /// Poll a condition until it succeeds or times out
    ///
    /// This is useful for testing eventual consistency across peers.
    ///
    /// # Arguments
    /// * `timeout` - Maximum time to wait
    /// * `condition` - Async function that takes &TestNetwork and returns Ok(true) when condition is met
    ///
    /// # Example
    /// ```rust,ignore
    /// let link_clone = link.clone();
    /// net.eventually(Duration::from_secs(5), || async {
    ///     let bob = net.peer("bob").unwrap();
    ///     bob.has_blob(&link_clone).await
    /// }).await?;
    /// ```
    pub async fn eventually<F, Fut>(&self, timeout: Duration, condition: F) -> Result<()>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<bool>>,
    {
        let start = std::time::Instant::now();
        let poll_interval = Duration::from_millis(100);

        loop {
            match condition().await {
                Ok(true) => {
                    tracing::debug!("Eventual condition met after {:?}", start.elapsed());
                    return Ok(());
                }
                Ok(false) => {
                    // Continue polling
                }
                Err(e) => {
                    tracing::debug!("Eventual condition check error: {}", e);
                    // Continue polling - transient errors are expected
                }
            }

            if start.elapsed() > timeout {
                return Err(anyhow::anyhow!(
                    "Condition not met within timeout ({:?})",
                    timeout
                ));
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Wait for a specific duration (helper for tests)
    pub async fn wait(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

impl Default for TestNetwork {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TestNetwork {
    fn drop(&mut self) {
        // Peers will be dropped and send shutdown signals
        tracing::debug!(
            "TestNetwork dropped, {} peers will be cleaned up",
            self.peers.len()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_network_lifecycle() -> Result<()> {
        let mut net = TestNetwork::new();

        // Add peers
        let _alice = net.add_peer("alice").await?;
        let _bob = net.add_peer("bob").await?;

        assert_eq!(net.peer_names().len(), 2);
        assert!(net.peer("alice").is_some());
        assert!(net.peer("bob").is_some());

        // Remove a peer
        net.remove_peer("alice").await?;
        assert_eq!(net.peer_names().len(), 1);
        assert!(net.peer("alice").is_none());

        // Shutdown
        net.shutdown().await?;
        assert_eq!(net.peer_names().len(), 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_eventually_success() -> Result<()> {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let net = TestNetwork::new();
        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = count.clone();

        net.eventually(Duration::from_secs(1), move || {
            let count = count_clone.clone();
            async move {
                let val = count.fetch_add(1, Ordering::SeqCst) + 1;
                Ok(val >= 3)
            }
        })
        .await?;

        assert!(count.load(Ordering::SeqCst) >= 3);

        Ok(())
    }

    #[tokio::test]
    async fn test_eventually_timeout() {
        let net = TestNetwork::new();

        let result = net
            .eventually(Duration::from_millis(200), || async { Ok(false) })
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timeout"));
    }
}
