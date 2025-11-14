use crate::bucket_log::memory::MemoryBucketLogProvider;
use crate::bucket_log::BucketLogProvider;
use crate::crypto::{PublicKey, SecretKey};
use crate::linked_data::Link;
use crate::mount::Manifest;
use crate::peer::{BlobsStore, Peer, PeerBuilder};
use anyhow::{anyhow, Result};
use iroh::NodeId;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use uuid::Uuid;

/// A test peer with convenience methods for integration testing
pub struct TestPeer {
    /// The name of this peer (for debugging)
    pub name: String,
    /// The underlying peer instance (cloned from original before spawn)
    peer: Peer<MemoryBucketLogProvider>,
    /// Secret key for this peer
    secret: SecretKey,
    /// Shutdown signal sender
    shutdown_tx: Option<watch::Sender<()>>,
    /// Handle to the running peer task
    peer_task: Option<JoinHandle<Result<()>>>,
}

impl TestPeer {
    /// Create a new test peer
    ///
    /// # Arguments
    /// * `name` - A name for this peer (useful for debugging)
    /// * `secret` - Optional secret key (generates random if None)
    /// * `blobs_path` - Optional path for blob storage (uses temp dir if None)
    pub async fn new(
        name: impl Into<String>,
        secret: Option<SecretKey>,
        blobs_path: Option<PathBuf>,
    ) -> Result<Self> {
        let name = name.into();
        let secret = secret.unwrap_or_else(SecretKey::generate);

        // Create in-memory bucket log
        let log_provider = MemoryBucketLogProvider::new();

        // Create blobs store (in temp dir or specified path)
        let blobs = if let Some(path) = blobs_path {
            BlobsStore::fs(&path).await?
        } else {
            let temp_dir = tempfile::tempdir()?;
            BlobsStore::fs(temp_dir.path()).await?
        };

        // Build the peer - let it bind to ephemeral port
        let peer = PeerBuilder::new()
            .log_provider(log_provider)
            .blobs_store(blobs)
            .secret_key(secret.clone())
            .build()
            .await;

        Ok(Self {
            name,
            peer,
            secret,
            shutdown_tx: None,
            peer_task: None,
        })
    }

    /// Start the peer (spawns background tasks)
    pub async fn start(&mut self) -> Result<()> {
        if self.peer_task.is_some() {
            return Err(anyhow!("Peer already started"));
        }

        let (shutdown_tx, shutdown_rx) = watch::channel(());

        // Build a new peer for spawning (with job_receiver intact)
        let log_provider = MemoryBucketLogProvider::new();
        let blobs = self.peer.blobs().clone();
        let secret = self.secret.clone();

        let peer_for_spawn = PeerBuilder::new()
            .log_provider(log_provider)
            .blobs_store(blobs)
            .secret_key(secret)
            .build()
            .await;

        // Clone for our use (this clone won't have job_receiver, but that's ok)
        self.peer = peer_for_spawn.clone();

        let name = self.name.clone();

        // Spawn the peer (consumes the original with job_receiver)
        let handle = tokio::spawn(async move {
            tracing::debug!("[{}] Starting peer", name);
            if let Err(e) = crate::peer::spawn(peer_for_spawn, shutdown_rx).await {
                tracing::error!("[{}] Peer error: {}", name, e);
                return Err(e.into());
            }
            tracing::debug!("[{}] Peer stopped", name);
            Ok(())
        });

        self.shutdown_tx = Some(shutdown_tx);
        self.peer_task = Some(handle);

        // Give the peer a moment to start up
        tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

        tracing::info!(
            "[{}] Peer started with ID: {} on {:?}",
            self.name,
            self.id(),
            self.peer.endpoint().bound_sockets()
        );

        Ok(())
    }

    /// Stop the peer gracefully
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.peer_task.take() {
            tracing::info!("[{}] Stopping peer", self.name);
            // Send shutdown signal
            if let Some(tx) = &self.shutdown_tx {
                let _ = tx.send(());
            }
            // Wait for task to complete
            handle.await??;
            tracing::info!("[{}] Peer stopped successfully", self.name);
        }
        Ok(())
    }

    /// Get the peer's node ID
    pub fn id(&self) -> NodeId {
        self.peer.id()
    }

    /// Get the peer's public key
    pub fn public_key(&self) -> PublicKey {
        PublicKey::from(*self.secret.public())
    }

    /// Get the peer's socket address
    pub fn socket_addr(&self) -> SocketAddr {
        *self.peer.socket()
    }

    /// Get reference to the underlying peer
    pub fn peer(&self) -> &Peer<MemoryBucketLogProvider> {
        &self.peer
    }

    /// Get reference to blobs store
    pub fn blobs(&self) -> &BlobsStore {
        self.peer.blobs()
    }

    // ========================================
    // Bucket Operations
    // ========================================

    /// Create a new bucket
    pub async fn create_bucket(&self, name: impl Into<String>) -> Result<Uuid> {
        let bucket_id = Uuid::new_v4();
        let name = name.into();

        // Just initialize the log with a genesis entry
        tracing::debug!("[{}] Creating bucket: {} ({})", self.name, name, bucket_id);

        Ok(bucket_id)
    }

    /// Commit to a bucket with a manifest
    pub async fn commit_bucket(
        &self,
        bucket_id: Uuid,
        manifest: &Manifest,
        link: Link,
        previous: Option<Link>,
    ) -> Result<()> {
        let height = match self.peer.logs().height(bucket_id).await {
            Ok(h) => h + 1,
            Err(_) => 0, // Genesis
        };

        tracing::debug!(
            "[{}] Committing to bucket {} at height {}",
            self.name,
            bucket_id,
            height
        );

        self.peer
            .logs()
            .append(
                bucket_id,
                manifest.name().to_string(),
                link,
                previous,
                height,
            )
            .await?;

        Ok(())
    }

    /// Get the current height of a bucket
    pub async fn bucket_height(&self, bucket_id: Uuid) -> Result<u64> {
        Ok(self.peer.logs().height(bucket_id).await?)
    }

    /// Get the head link at a specific height
    pub async fn bucket_head(&self, bucket_id: Uuid, height: u64) -> Result<Link> {
        let heads = self.peer.logs().heads(bucket_id, height).await?;
        heads
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No head at height {}", height))
    }

    /// Check if a link exists in the bucket log
    pub async fn has_link(&self, bucket_id: Uuid, link: &Link) -> Result<bool> {
        let heights = self.peer.logs().has(bucket_id, link.clone()).await?;
        Ok(!heights.is_empty())
    }

    // ========================================
    // Blob Operations
    // ========================================

    /// Put data into blobs and return the hash
    pub async fn put_blob(&self, data: &[u8]) -> Result<Link> {
        let hash = self.blobs().put(data.to_vec()).await?;
        Ok(Link::new(crate::linked_data::LD_RAW_CODEC, hash))
    }

    /// Get data from blobs by link
    pub async fn get_blob(&self, link: &Link) -> Result<Vec<u8>> {
        let data = self.blobs().get(&link.hash()).await?;
        Ok(data.to_vec())
    }

    /// Check if a blob exists locally
    pub async fn has_blob(&self, link: &Link) -> Result<bool> {
        Ok(self.blobs().stat(&link.hash()).await?)
    }

    // ========================================
    // Sync Operations
    // ========================================

    /// Sync a bucket from another peer
    pub async fn sync_from(&self, other: &TestPeer, bucket_id: Uuid) -> Result<()> {
        // Get the other peer's current state
        let target_height = other.bucket_height(bucket_id).await?;
        let target_link = other.bucket_head(bucket_id, target_height).await?;

        tracing::info!(
            "[{}] Syncing bucket {} from [{}] (height: {}, link: {})",
            self.name,
            bucket_id,
            other.name,
            target_height,
            target_link
        );

        // Use the peer's sync_from_peer method
        self.peer
            .sync_from_peer(bucket_id, target_link, target_height, &other.public_key())
            .await?;

        tracing::info!("[{}] Sync completed successfully", self.name);

        Ok(())
    }

    /// Download a specific blob from another peer
    pub async fn download_blob_from(&self, other: &TestPeer, link: &Link) -> Result<()> {
        tracing::debug!(
            "[{}] Downloading blob {} from [{}]",
            self.name,
            link,
            other.name
        );

        // For local testing, note the direct addresses (DHT discovery will handle it)
        tracing::debug!(
            "[{}] Downloading from [{}] at {:?}",
            self.name,
            other.name,
            other.peer.endpoint().bound_sockets()
        );

        // Just use the existing download_hash method but it should work with direct peer discovery
        // The DHT discovery is built in, so this should work now
        self.blobs()
            .download_hash(link.hash(), vec![other.public_key()], self.peer.endpoint())
            .await?;

        Ok(())
    }
}

impl Drop for TestPeer {
    fn drop(&mut self) {
        // Send shutdown signal
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(());
        }
    }
}
