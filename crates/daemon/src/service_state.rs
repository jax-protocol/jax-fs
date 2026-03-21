#[cfg(feature = "fuse")]
use std::sync::Arc;

#[cfg(feature = "fuse")]
use tokio::sync::RwLock;
use url::Url;

use crate::blobs::{Blobs, BlobsSetupError};
use crate::database::{Database, DatabaseSetupError};
#[cfg(feature = "fuse")]
use crate::fuse::{MountManager, MountManagerConfig};
use crate::http_server::gateway::cache::store::CacheStore;
use crate::http_server::gateway::cache::GatewayCache;
use crate::service_config::Config;
use crate::sync_provider::{QueuedSyncConfig, QueuedSyncProvider};

use common::crypto::SecretKey;
use common::peer::{Peer, PeerBuilder};

/// Main service state - orchestrates all components
#[derive(Clone)]
pub struct State {
    database: Database,
    peer: Peer<Database>,
    gateway_cache: Option<GatewayCache>,
    #[cfg(feature = "fuse")]
    mount_manager: Arc<RwLock<Option<MountManager>>>,
}

impl State {
    pub async fn from_config(config: &Config) -> Result<Self, StateSetupError> {
        // 1. Setup database
        let sqlite_database_url = match config.sqlite_path {
            Some(ref path) => {
                // check that the path exists
                if !path.exists() {
                    return Err(StateSetupError::DatabasePathDoesNotExist);
                }
                // parse the path into a URL
                Url::parse(&format!("sqlite://{}", path.display()))
                    .map_err(|_| StateSetupError::InvalidDatabaseUrl)
            }
            // otherwise just set up an in-memory database
            None => Url::parse("sqlite::memory:").map_err(|_| StateSetupError::InvalidDatabaseUrl),
        }?;
        tracing::info!("Database URL: {:?}", sqlite_database_url);
        let database = Database::connect(&sqlite_database_url).await?;

        // 2. Setup node secret
        let node_secret = config
            .node_secret
            .clone()
            .unwrap_or_else(SecretKey::generate);

        // 3. Setup blobs store using the new blobs module
        tracing::debug!("ServiceState::from_config - loading blobs store");
        let blobs =
            Blobs::setup(&config.blob_store, &config.jax_dir, config.max_import_size).await?;
        tracing::debug!("ServiceState::from_config - blobs store loaded successfully");

        // 4. Build peer from the database as the log provider
        // TODO: Make queue size configurable via config

        // Create sync provider with worker
        let (sync_provider, job_receiver) = QueuedSyncProvider::new(QueuedSyncConfig::default());

        let mut peer_builder = PeerBuilder::new()
            .with_sync_provider(std::sync::Arc::new(sync_provider))
            .log_provider(database.clone())
            .blobs_store(blobs.into_inner())
            .secret_key(node_secret.clone());

        if let Some(addr) = config.node_listen_addr {
            peer_builder = peer_builder.socket_address(addr);
        }

        let peer = peer_builder.build().await;

        // Log the bound addresses
        let bound_addrs = peer.endpoint().bound_sockets();
        tracing::info!("Node id: {} (with JAX protocol)", peer.id());
        tracing::info!("Peer listening on: {:?}", bound_addrs);

        // Spawn the worker for the queued sync provider
        // The worker is managed outside the peer, like the database
        let peer_for_worker = peer.clone();
        let job_stream = job_receiver.into_async();
        tokio::spawn(async move {
            crate::sync_provider::run_worker(peer_for_worker, job_stream).await;
        });

        // Initialize gateway cache
        let cache_dir = config.jax_dir.join("gateway-cache");
        let gateway_cache = match CacheStore::new_local(&cache_dir.join("blobs")).await {
            Ok(store) => {
                let cache = GatewayCache::spawn(
                    database.clone(),
                    store,
                    crate::http_server::gateway::cache::CacheConfig::default(),
                );
                tracing::info!("Gateway cache initialized at {:?}", cache_dir);
                Some(cache)
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to initialize gateway cache store, continuing without: {}",
                    e
                );
                None
            }
        };

        // Create the initial state
        let state = Self {
            database: database.clone(),
            peer: peer.clone(),
            gateway_cache,
            #[cfg(feature = "fuse")]
            mount_manager: Arc::new(RwLock::new(None)),
        };

        // Initialize mount manager with fuse feature
        #[cfg(feature = "fuse")]
        {
            let mount_manager = MountManager::new(
                database,
                peer,
                MountManagerConfig {
                    api_port: config.api_port,
                    ..MountManagerConfig::default()
                },
            );
            *state.mount_manager.write().await = Some(mount_manager);
        }

        Ok(state)
    }

    pub fn peer(&self) -> &Peer<Database> {
        &self.peer
    }

    pub fn node(&self) -> &Peer<Database> {
        // Alias for backwards compatibility
        &self.peer
    }

    pub fn database(&self) -> &Database {
        &self.database
    }

    pub fn gateway_cache(&self) -> Option<&GatewayCache> {
        self.gateway_cache.as_ref()
    }

    /// Get the mount manager (only available with fuse feature)
    #[cfg(feature = "fuse")]
    pub fn mount_manager(&self) -> &Arc<RwLock<Option<MountManager>>> {
        &self.mount_manager
    }
}

impl AsRef<Peer<Database>> for State {
    fn as_ref(&self) -> &Peer<Database> {
        &self.peer
    }
}

impl AsRef<Database> for State {
    fn as_ref(&self) -> &Database {
        self.database()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StateSetupError {
    #[error("Database path does not exist")]
    DatabasePathDoesNotExist,
    #[error("Database setup error")]
    DatabaseSetupError(#[from] DatabaseSetupError),
    #[error("Invalid database URL")]
    InvalidDatabaseUrl,
    #[error("Blobs setup error: {0}")]
    BlobsSetupError(#[from] BlobsSetupError),
}
