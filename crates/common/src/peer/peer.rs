use uuid::Uuid;

use crate::bucket_log_provider::BucketLogProvider;

use crate::crypto::SecretKey;
use crate::linked_data::Link;

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;

use futures::FutureExt;
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
pub struct PeerBuilder<L: BucketLogProvider> {
    /// the socket addr to expose the peer on
    ///  if not set, an ephemeral port will be used
    socket_addr: Option<SocketAddr>,
    /// the identity of the peer, as a SecretKey
    secret_key: Option<SecretKey>,
    /// pre-loaded blobs store (if provided, blobs_store_path is ignored)
    blobs_store: Option<BlobsStore>,
    log_provider: Option<L>,
}

// TODO (amiller68): proper errors
impl<L: BucketLogProvider> PeerBuilder<L> {
    pub fn new() -> Self {
        PeerBuilder {
            socket_addr: None,
            secret_key: None,
            blobs_store: None,
            log_provider: None,
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

    pub fn log_provider(mut self, log_provider: L) -> Self {
        self.log_provider = Some(log_provider);
        self
    }

    pub async fn build(self) -> Peer<L> {
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

        // get the log provider, must be set
        let log_provider = self
            .log_provider
            .expect("log_provider must be set");

        Peer {
            log_provider,
            socket_address: socket_addr,
            blobs_store,
            secret_key,
            endpoint,
        }
    }
}

/// Overview of a peer's state, generic over a bucket log provider.
///  Provides everything that a peer needs in order to
///  load data, interact with peers, and manage buckets.
#[derive(Debug, Clone)]
pub struct Peer<L: BucketLogProvider> {
    log_provider: L,
    socket_address: SocketAddr,
    blobs_store: BlobsStore,
    secret_key: SecretKey,
    endpoint: Endpoint,
}

impl<L: BucketLogProvider> Peer<L> {
    /// Create a Peer from a log provider and blobs store path
    pub fn from_state(log_provider: L, _blobs_store_path: PathBuf) -> Self {
        // Get necessary components from the bucket state provider
        // For now, we'll use default values - this should be updated based on the actual state interface
        let socket_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);
        let secret_key = SecretKey::generate();

        // Setup our discovery mechanism for our peer
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
            .now_or_never()
            .expect("failed to bind immediately")
            .expect("failed to bind ephemeral endpoint");

        // Create blobs store in memory for now
        let blobs_store = BlobsStore::memory()
            .now_or_never()
            .expect("failed to create blobs store immediately")
            .expect("failed to create blobs store");

        Peer {
            log_provider,
            socket_address: socket_addr,
            blobs_store,
            secret_key,
            endpoint,
        }
    }

    pub fn log_provider(&self) -> &L {
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
}
