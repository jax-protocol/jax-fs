use std::path::PathBuf;

use url::Url;

use super::config::Config;
use crate::daemon::database::{Database, DatabaseSetupError};
use crate::state::BlobStoreConfig;

use common::crypto::SecretKey;
use common::peer::{BlobsStore, Peer, PeerBuilder};

use super::sync_provider::{QueuedSyncConfig, QueuedSyncProvider};

/// Main service state - orchestrates all components
#[derive(Clone)]
pub struct State {
    database: Database,
    peer: Peer<Database>,
}

/// Setup the blob store based on configuration.
/// Returns both the legacy BlobsStore (for the Peer) and the path used.
async fn setup_blobs_store(config: &Config) -> Result<(BlobsStore, PathBuf), StateSetupError> {
    match &config.blob_store {
        BlobStoreConfig::Legacy => {
            // Legacy mode: use the jax_dir/blobs path or a temp directory
            let blobs_path = config
                .jax_dir
                .as_ref()
                .map(|d| d.join("blobs"))
                .unwrap_or_else(|| {
                    let temp_dir =
                        tempfile::tempdir().expect("failed to create temporary directory");
                    temp_dir.keep()
                });

            tracing::info!(path = %blobs_path.display(), "Using legacy iroh blob store");
            let blobs = BlobsStore::fs(&blobs_path)
                .await
                .map_err(|e| StateSetupError::BlobsStoreError(e.to_string()))?;

            Ok((blobs, blobs_path))
        }

        BlobStoreConfig::Filesystem { path } => {
            // Filesystem mode: use SQLite + local object storage
            let store_path = path
                .clone()
                .or_else(|| config.jax_dir.as_ref().map(|d| d.join("blobs-store")))
                .unwrap_or_else(|| {
                    let temp_dir =
                        tempfile::tempdir().expect("failed to create temporary directory");
                    temp_dir.keep()
                });

            tracing::info!(path = %store_path.display(), "Using filesystem blob store (SQLite + local objects)");

            // Create the new blob store for actual storage
            let _new_store = blobs_store::BlobStore::new_local(&store_path)
                .await
                .map_err(|e| StateSetupError::BlobsStoreError(e.to_string()))?;

            // For now, we still need a legacy BlobsStore for the Peer
            // Use the objects subdirectory which the new store also uses
            let legacy_path = store_path.join("legacy-blobs");
            let blobs = BlobsStore::fs(&legacy_path)
                .await
                .map_err(|e| StateSetupError::BlobsStoreError(e.to_string()))?;

            Ok((blobs, store_path))
        }

        BlobStoreConfig::S3 {
            endpoint,
            access_key,
            secret_key,
            bucket,
            region,
        } => {
            tracing::info!(endpoint = %endpoint, bucket = %bucket, "Using S3 blob store");

            // Determine SQLite path for metadata
            let db_path = config
                .jax_dir
                .as_ref()
                .map(|d| d.join("blobs-store.db"))
                .unwrap_or_else(|| PathBuf::from("/tmp/jax-blobs-store.db"));

            // Create the S3 object store config
            let s3_config = blobs_store::ObjectStoreConfig::S3 {
                endpoint: endpoint.clone(),
                access_key: access_key.clone(),
                secret_key: secret_key.clone(),
                bucket: bucket.clone(),
                region: region.clone(),
            };

            // Create the new blob store
            let _new_store = blobs_store::BlobStore::new(&db_path, s3_config)
                .await
                .map_err(|e| StateSetupError::BlobsStoreError(e.to_string()))?;

            // For now, we still need a legacy BlobsStore for the Peer
            // Use a temp directory since S3 mode doesn't have local storage
            let legacy_path = config
                .jax_dir
                .as_ref()
                .map(|d| d.join("legacy-blobs"))
                .unwrap_or_else(|| {
                    let temp_dir =
                        tempfile::tempdir().expect("failed to create temporary directory");
                    temp_dir.keep()
                });

            let blobs = BlobsStore::fs(&legacy_path)
                .await
                .map_err(|e| StateSetupError::BlobsStoreError(e.to_string()))?;

            Ok((blobs, legacy_path))
        }
    }
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
        tracing::debug!("ServiceState::from_config - loading blobs store");
        let (blobs, blobs_path) = setup_blobs_store(config).await?;
        tracing::debug!(path = %blobs_path.display(), "ServiceState::from_config - blobs store loaded successfully");

        // 4. Build peer from the database as the log provider
        // TODO: Make queue size configurable via config

        // Create sync provider with worker
        let (sync_provider, job_receiver) = QueuedSyncProvider::new(QueuedSyncConfig::default());

        let mut peer_builder = PeerBuilder::new()
            .with_sync_provider(std::sync::Arc::new(sync_provider))
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

        // Spawn the worker for the queued sync provider
        // The worker is managed outside the peer, like the database
        let peer_for_worker = peer.clone();
        let job_stream = job_receiver.into_async();
        tokio::spawn(async move {
            super::sync_provider::run_worker(peer_for_worker, job_stream).await;
        });

        Ok(Self { database, peer })
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
