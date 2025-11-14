use crate::crypto::{PublicKey, SecretKey};

use std::net::SocketAddr;

use anyhow::{anyhow, Result};
use iroh::{Endpoint, NodeId};
use uuid::Uuid;

pub use super::blobs_store::BlobsStore;

use crate::bucket_log::BucketLogProvider;
use crate::linked_data::Link;
use crate::mount::{Manifest, Mount, MountError};

/// Overview of a peer's state, generic over a bucket log provider.
///  Provides everything that a peer needs in order to
///  load data, interact with peers, and manage buckets.
#[derive(Debug)]
pub struct Peer<L: BucketLogProvider> {
    log_provider: L,
    socket_address: SocketAddr,
    blobs_store: BlobsStore,
    secret_key: SecretKey,
    endpoint: Endpoint,
    jobs: super::jobs::JobDispatcher,
    job_receiver: Option<super::jobs::JobReceiver>,
}

impl<L: BucketLogProvider> Clone for Peer<L>
where
    L: Clone,
{
    fn clone(&self) -> Self {
        Self {
            log_provider: self.log_provider.clone(),
            socket_address: self.socket_address,
            blobs_store: self.blobs_store.clone(),
            secret_key: self.secret_key.clone(),
            endpoint: self.endpoint.clone(),
            jobs: self.jobs.clone(),
            // JobReceiver cannot be cloned - only the original peer can spawn worker
            job_receiver: None,
        }
    }
}

impl<L: BucketLogProvider> Peer<L> {
    pub fn new(
        log_provider: L,
        socket_address: SocketAddr,
        blobs_store: BlobsStore,
        secret_key: SecretKey,
        endpoint: Endpoint,
        jobs: super::jobs::JobDispatcher,
        job_receiver: super::jobs::JobReceiver,
    ) -> Peer<L> {
        Self {
            log_provider,
            socket_address,
            blobs_store,
            secret_key,
            endpoint,
            jobs,
            job_receiver: Some(job_receiver),
        }
    }

    pub fn logs(&self) -> &L {
        &self.log_provider
    }

    pub fn blobs(&self) -> &BlobsStore {
        &self.blobs_store
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    pub fn secret(&self) -> &SecretKey {
        &self.secret_key
    }

    pub fn socket(&self) -> &SocketAddr {
        &self.socket_address
    }

    pub fn id(&self) -> NodeId {
        self.endpoint.node_id()
    }

    pub fn jobs(&self) -> &super::jobs::JobDispatcher {
        &self.jobs
    }

    /// Extract the job receiver (internal use by peer::spawn)
    ///
    /// This can only be called once. Subsequent calls will return None.
    pub(super) fn take_job_receiver(&mut self) -> Option<super::jobs::JobReceiver> {
        let receiver = self.job_receiver.take();
        if receiver.is_some() {
            tracing::info!("PEER: Successfully extracted job_receiver (was Some)");
        } else {
            tracing::warn!(
                "PEER: Failed to extract job_receiver (was None) - likely called on a clone"
            );
        }
        receiver
    }

    /// Load mount at the current head of a bucket
    ///
    /// # Arguments
    ///
    /// * `bucket_id` - The UUID of the bucket to load
    ///
    /// # Returns
    ///
    /// The Mount at the current head of the bucket's log
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Bucket not found in log
    /// - Failed to load mount from blobs
    pub async fn mount(&self, bucket_id: Uuid) -> Result<Mount, MountError> {
        // Get current head link from log
        let (link, _height) = self
            .log_provider
            .head(bucket_id, None)
            .await
            .map_err(|e| MountError::Default(anyhow!("Failed to get current head: {}", e)))?;

        // Load mount at that link (height is read from manifest)
        Mount::load(&link, &self.secret_key, &self.blobs_store).await
    }

    /// Save a mount and append it to the bucket's log
    ///
    /// This method:
    /// 1. Saves the mount to blobs, getting a new link
    /// 2. Appends the new link to the bucket's log
    /// 3. Dispatches sync jobs to notify peers
    ///
    /// # Arguments
    ///
    /// * `bucket_id` - The UUID of the bucket
    /// * `name` - The name of the bucket (for log metadata)
    /// * `mount` - The mount to save
    ///
    /// # Returns
    ///
    /// The new link where the mount was saved
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Failed to save mount to blobs
    /// - Failed to append to log
    pub async fn save_mount(&self, mount: &Mount) -> Result<Link, MountError> {
        // Get our own public key to exclude from notifications
        let our_public_key = self.secret_key.public();
        tracing::info!("SAVE_MOUNT: Our public key: {}", our_public_key.to_hex());

        let inner_mount = mount.inner().await;
        let manifest = inner_mount.manifest();

        let bucket_id = manifest.id().clone();
        let name = manifest.name().to_string();

        // Get shares from the mount manifest
        let (link, previous_link, height) = mount.save(self.blobs()).await?;
        let inner = mount.inner().await;
        let shares = inner.manifest().shares();
        tracing::info!("SAVE_MOUNT: Found {} shares in manifest", shares.len());

        // Append to log
        self.log_provider
            .append(bucket_id, name, link.clone(), Some(previous_link), height)
            .await
            .map_err(|e| MountError::Default(anyhow!("Failed to append to log: {}", e)))?;

        // Dispatch ping jobs for each peer (except ourselves)
        let mut notified_count = 0;
        for (peer_key_hex, _share) in shares.iter() {
            tracing::info!("SAVE_MOUNT: Checking share for peer: {}", peer_key_hex);

            // Parse the peer's public key
            if let Ok(peer_public_key) = PublicKey::from_hex(peer_key_hex) {
                // Skip ourselves
                if peer_public_key == our_public_key {
                    tracing::info!("SAVE_MOUNT: Skipping ourselves: {}", peer_key_hex);
                    continue;
                }

                tracing::info!(
                    "SAVE_MOUNT: Dispatching PingPeer job for bucket {} to peer {}",
                    bucket_id,
                    peer_key_hex
                );
                // Dispatch a ping job for this peer
                // Ignore errors - if we can't notify a peer, they'll catch up on their next ping
                let _ = self.jobs.dispatch(super::jobs::Job::PingPeer {
                    bucket_id,
                    peer_id: peer_public_key,
                });
                notified_count += 1;
            } else {
                tracing::warn!(
                    "SAVE_MOUNT: Failed to parse peer public key: {}",
                    peer_key_hex
                );
            }
        }

        tracing::info!(
            "SAVE_MOUNT: Dispatched {} PingPeer jobs for bucket {}",
            notified_count,
            bucket_id
        );

        Ok(link)
    }

    // ========================================
    // Peer-Centric Sync Functions
    // ========================================

    /// Sync a bucket from a peer
    ///
    /// This is the main entry point for syncing. It handles both cases:
    /// - Updating an existing bucket we already have
    /// - Cloning a new bucket we don't have yet
    pub async fn sync_from_peer(
        &self,
        bucket_id: Uuid,
        link: Link,
        height: u64,
        peer_id: &PublicKey,
    ) -> Result<()>
    where
        L::Error: std::error::Error + Send + Sync + 'static,
    {
        tracing::info!(
            "Syncing bucket {} from peer {} to link {:?} at height {}",
            bucket_id,
            peer_id.to_hex(),
            link,
            height
        );

        let exists: bool = self.log_provider.exists(bucket_id).await?;

        let common_ancestor = if exists {
            // find a common ancestor between our log and the
            //  link the peer advertised to us
            self.find_common_ancestor(bucket_id, &link, peer_id).await?
        } else {
            None
        };

        // TODO (amiller68): between finding the common ancestor and downloading the manifest chain
        //  there are redundant operations. We should optimize this.

        // if we know the bucket exists, but we did not find a common ancestor
        //  then we have diverged / are not talking about the same bucket
        // for now just log a warning and do nothing
        if exists && common_ancestor.is_none() {
            tracing::warn!("Bucket {} diverged from peer {:?}", bucket_id, peer_id);
            return Ok(());
        }

        // Determine between what links we should download manifests for
        let stop_link = if let Some(ancestor) = common_ancestor {
            Some(&ancestor.0.clone())
        } else {
            // No common ancestor - we'll sync everything from the target back to genesis
            tracing::info!(
                "No common ancestor for bucket {}, syncing from genesis",
                bucket_id
            );
            None
        };

        // now we know there is a valid list of manifests we should
        //  fetch and apply to our log

        // Download manifest chain from peer (from target back to common ancestor)
        let manifests = self
            .download_manifest_chain(&link, stop_link, peer_id)
            .await?;

        // TODO (amiller68): maybe theres an optimization here in that we should know
        //  we can exit earlier by virtue of finding a common ancestor which is just
        //  our current head
        if manifests.is_empty() {
            tracing::info!("No new manifests to sync, already up to date");
            return Ok(());
        };

        // Just check we are still included in the shares at the end of this,
        //  if not we wouldn't have gotten the ping, but we might as well just
        //  check
        if !self.verify_provenance(&manifests.last().unwrap().0)? {
            tracing::warn!("Provenance verification failed: our key not in bucket shares");
            return Ok(());
        }

        // apply the updates to the bucket
        self.apply_manifest_chain(bucket_id, &manifests).await?;

        // tracing::info!("Successfully cloned bucket {} from peer", bucket_id);
        Ok(())
    }

    /// Download a chain of manifests from a peer
    ///
    /// Walks backwards through the manifest chain via `previous` links.
    /// Stops when it reaches `stop_at` link (common ancestor) or genesis (no previous).
    ///
    /// Returns manifests in order from oldest to newest with their links and heights.
    async fn download_manifest_chain(
        &self,
        start_link: &Link,
        stop_link: Option<&Link>,
        // TODO (amiller68): this could use multi-peer download
        peer_id: &PublicKey,
    ) -> Result<Vec<(Manifest, Link)>> {
        tracing::debug!(
            "Downloading manifest chain from {:?}, stop_at: {:?}",
            start_link,
            stop_link
        );

        let mut manifests = Vec::new();
        let mut current_link = start_link.clone();

        // Download manifests walking backwards
        loop {
            // Download the manifest blob from peer
            self.blobs_store
                .download_hash(
                    current_link.hash().clone(),
                    vec![peer_id.clone()],
                    &self.endpoint,
                )
                .await
                .map_err(|e| {
                    anyhow!(
                        "Failed to download manifest {:?} from peer: {}",
                        current_link,
                        e
                    )
                })?;

            // Read and decode the manifest
            let manifest: Manifest = self.blobs_store.get_cbor(&current_link.hash()).await?;

            // Check if we should stop
            if let Some(stop_link) = stop_link {
                if &current_link == stop_link {
                    tracing::debug!("Reached stop_at link, stopping download");
                    break;
                }
            }

            manifests.push((manifest.clone(), current_link.clone()));

            // Check for previous link
            match manifest.previous() {
                Some(prev_link) => {
                    current_link = prev_link.clone();
                }
                None => {
                    // Reached genesis, stop
                    tracing::debug!("Reached genesis manifest, stopping download");
                    break;
                }
            }
        }

        // Reverse to get oldest-to-newest order
        manifests.reverse();

        tracing::debug!("Downloaded {} manifests", manifests.len());
        Ok(manifests)
    }

    /// Find common ancestor by downloading manifests from peer
    ///
    /// Starting from `start_link`, walks backwards through the peer's manifest chain,
    /// downloading each manifest and checking if it exists in our local log.
    /// Returns the first (most recent) link and height found in our log.
    ///
    /// # Arguments
    ///
    /// * `bucket_id` - The bucket to check against our local log
    /// * `link` - The starting point on the peer's chain (typically their head)
    /// * `peer_id` - The peer to download manifests from
    ///
    /// # Returns
    ///
    /// * `Ok(Some((link, height)))` - Found common ancestor with its link and height
    /// * `Ok(None)` - No common ancestor found (reached genesis without intersection)
    /// * `Err(_)` - Download or log access error
    async fn find_common_ancestor(
        &self,
        bucket_id: Uuid,
        link: &Link,
        peer_id: &PublicKey,
    ) -> Result<Option<(Link, u64)>>
    where
        L::Error: std::error::Error + Send + Sync + 'static,
    {
        tracing::debug!(
            "Finding common ancestor starting from {:?} with peer {}",
            link,
            peer_id.to_hex()
        );

        let mut current_link = link.clone();
        let mut manifests_checked = 0;

        loop {
            manifests_checked += 1;
            tracing::debug!(
                "Checking manifest {} at link {:?}",
                manifests_checked,
                current_link
            );

            // TODO (amiller68): this should build in memory
            //  but for now we just download it
            // Download the manifest from peer
            self.blobs_store
                .download_hash(
                    current_link.hash().clone(),
                    vec![peer_id.clone()],
                    &self.endpoint,
                )
                .await
                .map_err(|e| {
                    anyhow!(
                        "Failed to download manifest {:?} from peer: {}",
                        current_link,
                        e
                    )
                })?;

            // Read and decode the manifest
            let manifest: Manifest = self.blobs_store.get_cbor(&current_link.hash()).await?;
            let height = manifest.height();

            // Check if this link exists in our local log
            match self.log_provider.has(bucket_id, current_link.clone()).await {
                Ok(heights) if !heights.is_empty() => {
                    tracing::info!(
                        "Found common ancestor at link {:?} with height {} (in our log at heights {:?})",
                        current_link,
                        height,
                        heights
                    );
                    return Ok(Some((current_link, height)));
                }
                Ok(_) => {
                    // Link not in our log, check previous
                    tracing::debug!("Link {:?} not in our log, checking previous", current_link);
                }
                Err(e) => {
                    tracing::warn!("Error checking for link in log: {}", e);
                    // Continue checking previous links despite error
                }
            }

            // Move to previous link
            match manifest.previous() {
                Some(prev_link) => {
                    current_link = prev_link.clone();
                }
                None => {
                    // Reached genesis without finding common ancestor
                    tracing::debug!(
                        "Reached genesis after checking {} manifests, no common ancestor found",
                        manifests_checked
                    );
                    return Ok(None);
                }
            }
        }
    }

    /// Apply a chain of manifests to the log
    ///
    /// Appends each manifest to the log in order, starting from `start_height`.
    async fn apply_manifest_chain(
        &self,
        bucket_id: Uuid,
        manifests: &[(Manifest, Link)],
    ) -> Result<()>
    where
        L::Error: std::error::Error + Send + Sync + 'static,
    {
        tracing::info!("Applying {} manifests to log", manifests.len(),);

        for (_i, (manifest, link)) in manifests.iter().enumerate() {
            let previous = manifest.previous().clone();
            let height = manifest.height();

            tracing::info!(
                "Appending manifest to log: height={}, link={:?}, previous={:?}",
                height,
                link,
                previous
            );

            self.log_provider
                .append(
                    bucket_id,
                    manifest.name().to_string(),
                    link.clone(),
                    previous,
                    height,
                )
                .await
                .map_err(|e| anyhow!("Failed to append manifest at height {}: {}", height, e))?;

            let pins_link = manifest.pins().clone();
            let peer_ids = manifest
                .shares()
                .iter()
                .map(|share| share.1.principal().identity.clone())
                .collect();
            return self.jobs.dispatch_download_pins(pins_link, peer_ids);
        }

        tracing::info!("Successfully applied {} manifests to log", manifests.len());
        Ok(())
    }

    /// Verify that our PublicKey is in the manifest's shares
    fn verify_provenance(&self, manifest: &Manifest) -> Result<bool> {
        let our_pub_key = PublicKey::from(*self.secret_key.public());
        let our_pub_key_hex = our_pub_key.to_hex();

        // Check if our key is in the shares
        let is_authorized = manifest
            .shares()
            .iter()
            .any(|(key_hex, _share)| key_hex == &our_pub_key_hex);

        tracing::debug!(
            "Provenance check: our_key={}, authorized={}",
            our_pub_key_hex,
            is_authorized
        );

        Ok(is_authorized)
    }

    // ========================================
    // Background Job Worker
    // ========================================

    /// Schedule periodic pings to peers for all our buckets
    ///
    /// This queries all buckets, extracts peer IDs from manifest shares,
    /// and dispatches ping jobs for each peer.
    async fn schedule_periodic_pings(&self)
    where
        L::Error: std::error::Error + Send + Sync + 'static,
    {
        // Get all bucket IDs
        let bucket_ids = match self.log_provider.list_buckets().await {
            Ok(ids) => ids,
            Err(e) => {
                tracing::error!("Failed to list buckets for periodic pings: {}", e);
                return;
            }
        };

        tracing::debug!("Scheduling periodic pings for {} buckets", bucket_ids.len());

        // For each bucket, dispatch pings to peers in shares
        for bucket_id in bucket_ids {
            if let Err(e) = self.ping_bucket_peers(bucket_id).await {
                tracing::warn!("Failed to ping peers for bucket {}: {}", bucket_id, e);
            }
        }
    }

    /// Ping all peers listed in a bucket's manifest shares
    async fn ping_bucket_peers(&self, bucket_id: Uuid) -> Result<()>
    where
        L::Error: std::error::Error + Send + Sync + 'static,
    {
        // Get current head link
        let (head_link, _) = self
            .log_provider
            .head(bucket_id, None)
            .await
            .map_err(|e| anyhow!("Failed to get head for bucket {}: {}", bucket_id, e))?;

        // Load manifest from blobs store
        let manifest_bytes = self
            .blobs_store
            .get(&head_link.hash())
            .await
            .map_err(|e| anyhow!("Failed to load manifest: {}", e))?;

        let manifest: crate::prelude::Manifest = serde_ipld_dagcbor::from_slice(&manifest_bytes)
            .map_err(|e| anyhow!("Failed to deserialize manifest: {}", e))?;

        // Extract our own key to skip ourselves
        let our_key = crate::crypto::PublicKey::from(*self.secret_key.public()).to_hex();

        // For each peer in shares, dispatch a ping job
        for (peer_key_hex, _share) in manifest.shares() {
            if peer_key_hex == &our_key {
                continue; // Skip ourselves
            }

            let peer_id = crate::crypto::PublicKey::from_hex(peer_key_hex)
                .map_err(|e| anyhow!("Invalid peer key in shares: {}", e))?;

            // Dispatch ping job
            if let Err(e) = self
                .jobs
                .dispatch(super::jobs::Job::PingPeer { bucket_id, peer_id })
            {
                tracing::warn!(
                    "Failed to dispatch ping job for bucket {} peer {}: {}",
                    bucket_id,
                    peer_key_hex,
                    e
                );
            }
        }

        Ok(())
    }

    /// Handle a ping peer job by sending a ping and processing the response
    async fn handle_ping_peer(&self, bucket_id: Uuid, peer_id: PublicKey)
    where
        L::Error: std::error::Error + Send + Sync + 'static,
    {
        use super::protocol::bidirectional::BidirectionalHandler;
        use super::protocol::{Ping, PingMessage};

        // Get our bucket state
        let (our_link, our_height) = match self.log_provider.head(bucket_id, None).await {
            Ok((link, height)) => (link, height),
            Err(e) => {
                tracing::warn!(
                    "Failed to get head for bucket {} when pinging peer {}: {}",
                    bucket_id,
                    peer_id.to_hex(),
                    e
                );
                return;
            }
        };

        // Construct ping
        let ping = PingMessage {
            bucket_id,
            link: our_link,
            height: our_height,
        };

        // Send ping
        tracing::info!("Sending ping to peer {}", peer_id.to_hex());
        match Ping::send::<L>(&self, &peer_id, ping).await {
            Ok(pong) => {
                tracing::info!(
                    "Received pong from peer {} for bucket {} | {:?}",
                    peer_id.to_hex(),
                    bucket_id,
                    pong
                );
            }
            Err(e) => {
                tracing::debug!(
                    "Failed to ping peer {} for bucket {}: {}",
                    peer_id.to_hex(),
                    bucket_id,
                    e
                );
            }
        }
    }

    /// Run the background job worker
    ///
    /// This consumes the job receiver and processes jobs until the receiver is closed.
    /// Typically this should be spawned in a background task:
    ///
    /// ```ignore
    /// let (peer, job_receiver) = PeerBuilder::new()
    ///     .log_provider(database)
    ///     .build()
    ///     .await;
    ///
    /// // Spawn the worker
    /// tokio::spawn(async move {
    ///     peer.clone().run_worker(job_receiver).await;
    /// });
    /// ```
    pub async fn run_worker(self, job_receiver: super::jobs::JobReceiver)
    where
        L::Error: std::error::Error + Send + Sync + 'static,
    {
        use super::jobs::Job;
        use futures::StreamExt;
        use tokio::time::{interval, Duration};

        tracing::error!(
            "RUN_WORKER CALLED for peer {} - receiver is being held here",
            self.id()
        );

        // Convert to async stream for efficient async processing
        let mut stream = job_receiver.into_async();

        // Create interval timer for periodic pings (every 60 seconds)
        let mut ping_interval = interval(Duration::from_secs(5));
        ping_interval.tick().await; // Skip first immediate tick

        loop {
            tokio::select! {
                // Process incoming jobs from the queue
                Some(job) = stream.next() => {
                    match job {
                        Job::DownloadPins {
                            pins_link,
                            peer_ids,
                        } => {
                            if let Err(e) = self.blobs().download_hash_list(pins_link.hash(), peer_ids, self.endpoint()).await {
                                tracing::error!("Failed to download pins: {}", e);
                            }
                        },
                        Job::SyncBucket {
                            bucket_id,
                            target_link,
                            target_height,
                            peer_id,
                        } => {
                            tracing::info!(
                                "Processing sync job: bucket_id={}, peer_id={}, height={}",
                                bucket_id,
                                peer_id.to_hex(),
                                target_height
                            );

                            if let Err(e) = self
                                .sync_from_peer(bucket_id, target_link, target_height, &peer_id)
                                .await
                            {
                                tracing::error!(
                                    "Sync job failed for bucket {} from peer {}: {}",
                                    bucket_id,
                                    peer_id.to_hex(),
                                    e
                                );
                            } else {
                                tracing::info!(
                                    "Sync job completed successfully for bucket {} from peer {}",
                                    bucket_id,
                                    peer_id.to_hex()
                                );
                            }
                        }
                        Job::PingPeer { bucket_id, peer_id } => {
                            tracing::info!(
                                "JOB_WORKER: Processing PingPeer job: bucket_id={}, peer_id={}",
                                bucket_id,
                                peer_id.to_hex()
                            );
                            self.handle_ping_peer(bucket_id, peer_id).await;
                            tracing::info!(
                                "JOB_WORKER: Completed PingPeer job for bucket {} to peer {}",
                                bucket_id,
                                peer_id.to_hex()
                            );
                        }
                    }
                }

                // Periodic ping scheduler
                _ = ping_interval.tick() => {
                    tracing::info!("Running periodic ping scheduler");
                    self.schedule_periodic_pings().await;
                }

                // Stream closed (all senders dropped)
                else => {
                    tracing::info!("Job queue closed, shutting down worker");
                    break;
                }
            }
        }

        tracing::info!("Background job worker shutting down for peer {}", self.id());
    }
}
