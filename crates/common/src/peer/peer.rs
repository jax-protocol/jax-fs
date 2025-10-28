use uuid::Uuid;

use crate::bucket_state_provider::{BucketLogProvider, BucketSyncStatus};

use crate::crypto::SecretKey;
use crate::linked_data::Link;

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;

use iroh::discovery::pkarr::dht::DhtDiscovery;
use iroh::{protocol::Router, Endpoint, NodeId};
use tokio::sync::watch::Receiver as WatchReceiver;

pub use super::blobs_store::{BlobsStore, BlobsStoreError};
// pub use super::protocol::{
//     announce_to_peer, fetch_bucket, ping_peer, JaxProtocol, PeerStateProvider, PingRequest,
//     PingResponse, ShareInfo, SyncStatus, JAX_ALPN,
// };

// Re-export iroh types for convenience
pub use iroh::NodeAddr;

#[derive(Clone, Default)]
pub struct PeerBuilder<BucketStateProvider> {
    /// the socket addr to expose the peer on
    ///  if not set, an ephemeral port will be used
    socket_addr: Option<SocketAddr>,
    /// the identity of the peer, as a SecretKey
    secret_key: Option<SecretKey>,
    /// pre-loaded blobs store (if provided, blobs_store_path is ignored)
    blobs_store: Option<BlobsStore>,
    bucket_state_provider: Option<BucketStateProvider>,
}

// TODO (amiller68): proper errors
impl<BucketStateProvider> PeerBuilder<BucketStateProvider> {
    pub fn new() -> Self {
        PeerBuilder {
            socket_addr: None,
            secret_key: None,
            blobs_store: None,
            bucket_state_provider: None,
        }
    }

    pub fn socket_addr(mut self, socket_addr: SocketAddr) -> Self {
        self.socket_addr = Some(socket_addr);
        self
    }

    pub fn secret_key(mut self, secret_key: SecretKey) -> Self {
        self.secret_key = Some(secret_key);
        self
    }

    pub fn blobs_store(mut self, blobs: BlobsStore) -> Self {
        self.blobs_store = Some(blobs);
        self
    }

    pub fn bucket_state_provider(mut self, bucket_state_provider: BucketStateProvider) -> Self {
        self.bucket_state_provider = Some(bucket_state_provider);
        self
    }

    pub async fn build(self) -> Peer<BucketStateProvider> {
        // set the socket port to unspecified if not set
        let socket_addr = self
            .socket_addr
            .unwrap_or_else(|| SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0));
        // generate a new secret key if not set
        let secret_key = self.secret_key.unwrap_or_else(SecretKey::generate);

        // get the blobs store, if not set use in memory
        let blobs_store = match self.blobs_store {
            Some(blobs) => blobs,
            None => BlobsStore::memory().await.unwrap(),
        };

        // setup our discovery mechanism for our peer
        let mainline_discovery = DhtDiscovery::builder()
            .secret_key(secret_key.0.clone())
            .build()
            .expect("failed to build mainline discovery");

        // Convert the SocketAddr to a SocketAddrV4
        let addr = SocketAddrV4::new(
            socket_addr
                .ip()
                .to_string()
                .parse::<Ipv4Addr>()
                .expect("failed to parse IP address"),
            socket_addr.port(),
        );

        // Create the endpoint with our key and discovery
        let endpoint = Endpoint::builder()
            .secret_key(secret_key.0.clone())
            .discovery(mainline_discovery)
            .bind_addr_v4(addr)
            .bind()
            .await
            .expect("failed to bind ephemeral endpoint");

        // get the bucket state provider, must be set
        let bucket_state_provider = self
            .bucket_state_provider
            .expect("bucket_state_provider must be set");

        Peer {
            bucket_state_provider,
            socket_address: socket_addr,
            blobs_store,
            secret_key,
            endpoint,
        }
    }
}

/// Overview of a peer's state, generic over a bucket state provider.
///  Provides everything that a peer needs in order to
///  load data, interact with peers, and manage buckets.
#[derive(Debug)]
pub struct Peer<BucketStateProvider> {
    bucket_state_provider: BucketStateProvider,
    socket_address: SocketAddr,
    blobs_store: BlobsStore,
    secret_key: SecretKey,
    endpoint: Endpoint,
}

impl<BucketStateProvider> Peer<BucketStateProvider> {
    pub fn bucket_state(&self) -> &BucketStateProvider {
        &self.bucket_state_provider
    }

    pub fn blobs(&self) -> &BlobsStore {
        &self.blobs_store
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    fn secret(&self) -> &SecretKey {
        &self.secret_key
    }

    pub fn socket(&self) -> &SocketAddr {
        &self.socket_address
    }
}
