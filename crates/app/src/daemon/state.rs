use url::Url;

use super::config::Config;
use crate::daemon::database::{Database, DatabaseSetupError};

use common::crypto::SecretKey;
use common::peer::{BlobsStore, Peer, PeerBuilder};

/// Main service state - orchestrates all components
#[derive(Clone)]
pub struct State {
    database: Database,
    peer: Peer<Database>,
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

        // 3. Setup blobs store
        let blobs_store_path = config.node_blobs_store_path.clone().unwrap_or_else(|| {
            let temp_dir = tempfile::tempdir().expect("failed to create temporary directory");
            temp_dir.path().to_path_buf()
        });
        tracing::debug!("ServiceState::from_config - loading blobs store");
        let blobs = BlobsStore::fs(&blobs_store_path)
            .await
            .map_err(|e| StateSetupError::BlobsStoreError(e.to_string()))?;
        tracing::debug!("ServiceState::from_config - blobs store loaded successfully");

        // 4. Build peer from the database as the log provider
        let mut peer_builder = PeerBuilder::new()
            .log_provider(database.clone())
            .blobs_store(blobs.clone())
            .secret_key(node_secret.clone());

        if let Some(addr) = config.node_listen_addr {
            peer_builder = peer_builder.socket_address(addr);
        }

        let peer = peer_builder.build().await;

        // Log the bound addresses
        let bound_addrs = peer.endpoint().bound_sockets();
        tracing::info!("Node id: {} (with JAX protocol)", peer.id());
        tracing::info!("Peer listening on: {:?}", bound_addrs);

        Ok(Self { database, peer })
    }

    pub fn peer(&self) -> &Peer<Database> {
        &self.peer
    }

    pub fn node(&self) -> &Peer<Database> {
        // Alias for backwards compatibility
        &self.peer
    }

    /// Take ownership of the peer (can only be called once before State is Arc'd)
    ///
    /// This is used during startup to extract the peer for spawning the worker,
    /// which requires ownership (not a clone) to access the job_receiver.
    pub fn take_peer(&mut self) -> Peer<Database> {
        // Clone first, then replace
        let peer_clone = self.peer.clone();
        std::mem::replace(&mut self.peer, peer_clone)
    }

    pub fn database(&self) -> &Database {
        &self.database
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
    #[error("Blobs store error: {0}")]
    BlobsStoreError(String),
}
