use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use iroh::discovery::pkarr::dht::DhtDiscovery;
use iroh::Endpoint;

pub use super::blobs_store::BlobsStore;

use crate::bucket_log::BucketLogProvider;
use crate::crypto::SecretKey;

use super::peer::Peer;

#[derive(Clone, Default)]
pub struct PeerBuilder<L: BucketLogProvider> {
    /// the socket addr to expose the peer on
    ///  if not set, an ephemeral port will be used
    socket_address: Option<SocketAddr>,
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
            socket_address: None,
            secret_key: None,
            blobs_store: None,
            log_provider: None,
        }
    }

    pub fn socket_address(mut self, socket_addr: SocketAddr) -> Self {
        self.socket_address = Some(socket_addr);
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
            .socket_address
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
        let log_provider = self.log_provider.expect("log_provider is required");

        // Create the job dispatcher and receiver
        let (jobs, job_receiver) = super::jobs::JobDispatcher::new();

        Peer::new(
            log_provider,
            socket_addr,
            blobs_store,
            secret_key,
            endpoint,
            jobs,
            job_receiver,
        )
    }
}
