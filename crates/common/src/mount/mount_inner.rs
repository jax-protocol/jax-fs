use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Mutex;
use uuid::Uuid;

use crate::crypto::{PublicKey, Secret, SecretError, SecretKey, SecretShare};
use crate::linked_data::{BlockEncoded, CodecError, Link};
use crate::peer::{BlobsStore, BlobsStoreError};

use super::conflict::MergeResult;
use super::manifest::{Manifest, ManifestError, Share};
use super::node::{Node, NodeError, NodeLink};
use super::path_ops::{OpType, PathOpLog};
use super::pins::Pins;
use super::principal::PrincipalRole;

pub fn clean_path(path: &Path) -> PathBuf {
    if !path.is_absolute() {
        panic!("path is not absolute");
    }
    path.iter()
        .skip(1)
        .map(|part| part.to_string_lossy().to_string())
        .collect::<PathBuf>()
}

#[derive(Clone)]
pub struct MountInner {
    // link to the manifest
    pub link: Link,
    // the loaded manifest
    pub manifest: Manifest,
    // the loaded, decrypted entry node
    pub entry: Node,
    // the loaded pins
    pub pins: Pins,
    // convenience pointer to the height of the mount
    pub height: u64,
    // the path operations log (CRDT for conflict resolution)
    pub ops_log: PathOpLog,
    // the local peer ID (for recording operations)
    pub peer_id: PublicKey,
    // the secret key for signing manifests
    pub secret_key: SecretKey,
}

impl MountInner {
    pub fn link(&self) -> &Link {
        &self.link
    }
    pub fn entry(&self) -> &Node {
        &self.entry
    }
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn pins(&self) -> &Pins {
        &self.pins
    }
    pub fn height(&self) -> u64 {
        self.height
    }
    pub fn ops_log(&self) -> &PathOpLog {
        &self.ops_log
    }
    pub fn peer_id(&self) -> &PublicKey {
        &self.peer_id
    }
}

#[derive(Clone)]
pub struct Mount(Arc<Mutex<MountInner>>, BlobsStore);

#[derive(Debug, thiserror::Error)]
pub enum MountError {
    #[error("default error: {0}")]
    Default(#[from] anyhow::Error),
    #[error("link not found")]
    LinkNotFound(Link),
    #[error("path not found: {0}")]
    PathNotFound(PathBuf),
    #[error("path is not a node: {0}")]
    PathNotNode(PathBuf),
    #[error("path already exists: {0}")]
    PathAlreadyExists(PathBuf),
    #[error("cannot move '{from}' to '{to}': destination is inside source")]
    MoveIntoSelf { from: PathBuf, to: PathBuf },
    #[error("blobs store error: {0}")]
    BlobsStore(#[from] BlobsStoreError),
    #[error("secret error: {0}")]
    Secret(#[from] SecretError),
    #[error("node error: {0}")]
    Node(#[from] NodeError),
    #[error("codec error: {0}")]
    Codec(#[from] CodecError),
    #[error("share error: {0}")]
    Share(#[from] crate::crypto::SecretShareError),
    #[error("manifest error: {0}")]
    Manifest(#[from] ManifestError),
    #[error("peers share was not found")]
    ShareNotFound,
    #[error("mirror cannot mount: bucket is not published")]
    MirrorCannotMount,
}

impl Mount {
    pub async fn inner(&self) -> MountInner {
        self.0.lock().await.clone()
    }

    pub fn blobs(&self) -> BlobsStore {
        self.1.clone()
    }

    pub async fn link(&self) -> Link {
        let inner = self.0.lock().await;
        inner.link.clone()
    }

    /// Save the current mount state to the blobs store.
    ///
    /// If `publish` is true, the secret will be stored in plaintext, allowing
    /// mirrors to decrypt the bucket contents.
    pub async fn save(
        &self,
        blobs: &BlobsStore,
        publish: bool,
    ) -> Result<(Link, Link, u64), MountError> {
        // Clone data we need before any async operations
        let (
            entry_node,
            mut pins,
            previous_link,
            previous_height,
            manifest_template,
            ops_log,
            secret_key,
        ) = {
            let inner = self.0.lock().await;
            (
                inner.entry.clone(),
                inner.pins.clone(),
                inner.link.clone(),
                inner.height,
                inner.manifest.clone(),
                inner.ops_log.clone(),
                inner.secret_key.clone(),
            )
        };

        // Increment the height of the mount
        let height = previous_height + 1;

        // Create a new secret for the updated root
        let secret = Secret::generate();

        // Put the current root node into blobs with the new secret
        let entry = Self::_put_node_in_blobs(&entry_node, &secret, blobs).await?;

        // Serialize current pins to blobs
        // put the new root link into the pins, as well as the previous link
        pins.insert(entry.clone().hash());
        pins.insert(previous_link.hash());

        // Encrypt and store the ops log if it has any operations
        let ops_log_link = if !ops_log.is_empty() {
            let link = Self::_put_ops_log_in_blobs(&ops_log, &secret, blobs).await?;
            pins.insert(link.hash());
            Some(link)
        } else {
            None
        };

        let pins_link = Self::_put_pins_in_blobs(&pins, blobs).await?;

        // Re-encrypt owner shares with the new secret (mirrors stay unchanged)
        let mut manifest = manifest_template;
        for share in manifest.shares_mut().values_mut() {
            if *share.role() == PrincipalRole::Owner {
                let secret_share = SecretShare::new(&secret, &share.principal().identity)?;
                share.set_share(secret_share);
            }
        }

        // Update publish state: publish with new secret, or clear stale public secret
        if publish {
            manifest.publish(&secret);
        } else {
            // Clear any existing public secret since it would be stale
            // (encrypted with old secret, not the new one)
            manifest.unpublish();
        }
        manifest.set_pins(pins_link.clone());
        manifest.set_previous(previous_link.clone());
        manifest.set_entry(entry.clone());
        manifest.set_height(height);

        // Clear inherited ops_log from the template, then set if we have new operations
        // Each version's ops_log is independent and encrypted with that version's secret
        manifest.clear_ops_log();
        if let Some(ops_link) = ops_log_link {
            manifest.set_ops_log(ops_link);
        }

        // Sign the manifest with the stored secret key
        manifest.sign(&secret_key)?;

        // Put the updated manifest into blobs to determine the new link
        let link = Self::_put_manifest_in_blobs(&manifest, blobs).await?;

        // Update the internal state
        {
            let mut inner = self.0.lock().await;
            inner.manifest = manifest;
            inner.height = height;
            inner.link = link.clone();
            // Clear the ops_log - it's now persisted in the manifest
            // Future operations start a fresh log for the next version
            // IMPORTANT: Preserve the clock value so future ops have unique timestamps
            inner.ops_log.clear_preserving_clock();
        }

        Ok((link, previous_link, height))
    }

    pub async fn init(
        id: Uuid,
        name: String,
        owner: &SecretKey,
        blobs: &BlobsStore,
    ) -> Result<Self, MountError> {
        // create a new root node for the bucket
        let entry = Node::default();
        // create a new secret for the owner
        let secret = Secret::generate();
        // put the node in the blobs store for the secret
        let entry_link = Self::_put_node_in_blobs(&entry, &secret, blobs).await?;
        // share the secret with the owner
        let share = SecretShare::new(&secret, &owner.public())?;
        // Initialize pins with root node hash
        let mut pins = Pins::new();
        pins.insert(entry_link.hash());
        // Put the pins in blobs to get a pins link
        let pins_link = Self::_put_pins_in_blobs(&pins, blobs).await?;
        // construct the new manifest
        let mut manifest = Manifest::new(
            id,
            name.clone(),
            owner.public(),
            share,
            entry_link.clone(),
            pins_link.clone(),
            0, // initial height is 0
        );
        // Sign the manifest with the owner's key
        manifest.sign(owner)?;
        let link = Self::_put_manifest_in_blobs(&manifest, blobs).await?;

        // return the new mount
        Ok(Mount(
            Arc::new(Mutex::new(MountInner {
                link,
                manifest,
                entry,
                pins,
                height: 0,
                ops_log: PathOpLog::new(),
                peer_id: owner.public(),
                secret_key: owner.clone(),
            })),
            blobs.clone(),
        ))
    }

    pub async fn load(
        link: &Link,
        secret_key: &SecretKey,
        blobs: &BlobsStore,
    ) -> Result<Self, MountError> {
        let public_key = &secret_key.public();
        let manifest = Self::_get_manifest_from_blobs(link, blobs).await?;

        let bucket_share = match manifest.get_share(public_key) {
            Some(share) => share,
            None => return Err(MountError::ShareNotFound),
        };

        // Get the secret based on role
        let secret = match bucket_share.role() {
            PrincipalRole::Owner => {
                // Owners decrypt their individual share
                let share = bucket_share.share().ok_or(MountError::ShareNotFound)?;
                share.recover(secret_key)?
            }
            PrincipalRole::Mirror => {
                // Mirrors use the public secret (if bucket is published)
                manifest
                    .public()
                    .cloned()
                    .ok_or(MountError::MirrorCannotMount)?
            }
        };

        let pins = Self::_get_pins_from_blobs(manifest.pins(), blobs).await?;
        let entry = Self::_get_node_from_blobs(
            &NodeLink::Dir(manifest.entry().clone(), secret.clone()),
            blobs,
        )
        .await?;

        // Read height from the manifest
        let height = manifest.height();

        // Load the ops log if it exists, otherwise create a new one
        let ops_log = if let Some(ops_link) = manifest.ops_log() {
            let mut log = Self::_get_ops_log_from_blobs(ops_link, &secret, blobs).await?;
            // Rebuild local clock from operations after deserialization
            log.rebuild_clock();
            log
        } else {
            PathOpLog::new()
        };

        Ok(Mount(
            Arc::new(Mutex::new(MountInner {
                link: link.clone(),
                manifest,
                entry,
                pins,
                height,
                ops_log,
                peer_id: secret_key.public(),
                secret_key: secret_key.clone(),
            })),
            blobs.clone(),
        ))
    }

    /// Load just the manifest from a link without full mount decryption.
    ///
    /// This is useful for checking roles/shares before deciding which version to load.
    /// Returns the manifest if the blob exists, without attempting decryption.
    pub async fn load_manifest(link: &Link, blobs: &BlobsStore) -> Result<Manifest, MountError> {
        Self::_get_manifest_from_blobs(link, blobs).await
    }

    /// Add an owner to this bucket.
    /// Owners get an encrypted share immediately.
    pub async fn add_owner(&mut self, peer: PublicKey) -> Result<(), MountError> {
        let mut inner = self.0.lock().await;
        let secret_share = SecretShare::new(&Secret::default(), &peer)?;
        inner
            .manifest
            .add_share(Share::new_owner(secret_share, peer));
        Ok(())
    }

    /// Add a mirror to this bucket.
    /// Mirrors can sync bucket data but cannot decrypt until published.
    pub async fn add_mirror(&mut self, peer: PublicKey) {
        let mut inner = self.0.lock().await;
        inner.manifest.add_share(Share::new_mirror(peer));
    }

    /// Check if this bucket is published (mirrors can decrypt).
    pub async fn is_published(&self) -> bool {
        let inner = self.0.lock().await;
        inner.manifest.is_published()
    }

    /// Save and publish this bucket, granting decryption access to all mirrors.
    ///
    /// This is a convenience method equivalent to `save(blobs, true)`.
    pub async fn publish(&self) -> Result<(Link, Link, u64), MountError> {
        self.save(&self.1, true).await
    }

    pub async fn add<R>(&mut self, path: &Path, data: R) -> Result<(), MountError>
    where
        R: Read + Send + Sync + 'static + Unpin,
    {
        let secret = Secret::generate();

        let encrypted_reader = secret.encrypt_reader(data)?;

        // TODO (amiller68): this is incredibly dumb
        use bytes::Bytes;
        use futures::stream;
        let encrypted_bytes = {
            let mut buf = Vec::new();
            let mut reader = encrypted_reader;
            reader.read_to_end(&mut buf).map_err(SecretError::Io)?;
            buf
        };

        let stream = Box::pin(stream::once(async move {
            Ok::<_, std::io::Error>(Bytes::from(encrypted_bytes))
        }));

        let hash = self.1.put_stream(stream).await?;

        let link = Link::new(crate::linked_data::LD_RAW_CODEC, hash);

        let node_link = NodeLink::new_data_from_path(link.clone(), secret, path);

        let root_node = {
            let inner = self.0.lock().await;
            inner.entry.clone()
        };

        let (updated_link, node_hashes) =
            Self::_set_node_link_at_path(root_node, node_link, path, &self.1).await?;

        // Update entry if needed
        let new_entry = if let NodeLink::Dir(new_root_link, new_secret) = updated_link {
            Some(
                Self::_get_node_from_blobs(
                    &NodeLink::Dir(new_root_link.clone(), new_secret),
                    &self.1,
                )
                .await?,
            )
        } else {
            None
        };

        // Update inner state
        {
            let mut inner = self.0.lock().await;
            // Track pins: data blob + all created node hashes
            inner.pins.insert(hash);
            inner.pins.extend(node_hashes);

            if let Some(entry) = new_entry {
                inner.entry = entry;
            }

            // Record the add operation in the ops log
            let peer_id = inner.peer_id;
            inner
                .ops_log
                .record(peer_id, OpType::Add, clean_path(path), Some(link), false);
        }

        Ok(())
    }

    pub async fn rm(&mut self, path: &Path) -> Result<(), MountError> {
        let path = clean_path(path);
        let parent_path = path
            .parent()
            .ok_or_else(|| MountError::Default(anyhow::anyhow!("Cannot remove root")))?;

        let entry = {
            let inner = self.0.lock().await;
            inner.entry.clone()
        };

        let mut parent_node = if parent_path == Path::new("") {
            entry.clone()
        } else {
            Self::_get_node_at_path(&entry, parent_path, &self.1).await?
        };

        let file_name = path.file_name().unwrap().to_string_lossy().to_string();

        // Get the node link before deleting to check if it's a directory
        let removed_link = parent_node.del(&file_name);
        if removed_link.is_none() {
            return Err(MountError::PathNotFound(path.to_path_buf()));
        }
        let is_dir = removed_link.map(|l| l.is_dir()).unwrap_or(false);

        // Store path for ops log before we lose ownership
        let removed_path = path.to_path_buf();

        if parent_path == Path::new("") {
            let secret = Secret::generate();
            let link = Self::_put_node_in_blobs(&parent_node, &secret, &self.1).await?;

            let mut inner = self.0.lock().await;
            // Track the new root node hash
            inner.pins.insert(link.hash());
            inner.entry = parent_node;
        } else {
            // Save the modified parent node to blobs
            let secret = Secret::generate();
            let parent_link = Self::_put_node_in_blobs(&parent_node, &secret, &self.1).await?;
            let node_link = NodeLink::new_dir(parent_link.clone(), secret);

            // Convert parent_path back to absolute for _set_node_link_at_path
            let abs_parent_path = Path::new("/").join(parent_path);
            let (updated_link, node_hashes) =
                Self::_set_node_link_at_path(entry, node_link, &abs_parent_path, &self.1).await?;

            let new_entry = if let NodeLink::Dir(new_root_link, new_secret) = updated_link {
                Some(
                    Self::_get_node_from_blobs(
                        &NodeLink::Dir(new_root_link.clone(), new_secret),
                        &self.1,
                    )
                    .await?,
                )
            } else {
                None
            };

            let mut inner = self.0.lock().await;
            // Track the parent node hash and all created node hashes
            inner.pins.insert(parent_link.hash());
            inner.pins.extend(node_hashes);

            if let Some(new_entry) = new_entry {
                inner.entry = new_entry;
            }
        }

        // Record the remove operation in the ops log
        {
            let mut inner = self.0.lock().await;
            let peer_id = inner.peer_id;
            inner
                .ops_log
                .record(peer_id, OpType::Remove, removed_path, None, is_dir);
        }

        Ok(())
    }

    pub async fn mkdir(&mut self, path: &Path) -> Result<(), MountError> {
        let path = clean_path(path);

        // Check if the path already exists
        let entry = {
            let inner = self.0.lock().await;
            inner.entry.clone()
        };

        // Try to get the parent path and file name
        let (parent_path, dir_name) = if let Some(parent) = path.parent() {
            (
                parent,
                path.file_name().unwrap().to_string_lossy().to_string(),
            )
        } else {
            return Err(MountError::Default(anyhow::anyhow!("Cannot mkdir at root")));
        };

        // Get the parent node (or use root if parent is empty)
        let parent_node = if parent_path == Path::new("") {
            entry.clone()
        } else {
            // Check if parent path exists, if not it will be created by _set_node_link_at_path
            match Self::_get_node_at_path(&entry, parent_path, &self.1).await {
                Ok(node) => node,
                Err(MountError::PathNotFound(_)) => Node::default(), // Will be created
                Err(err) => return Err(err),
            }
        };

        // Check if a node with this name already exists in the parent
        if parent_node.get_link(&dir_name).is_some() {
            return Err(MountError::PathAlreadyExists(Path::new("/").join(&path)));
        }

        // Create an empty directory node
        let new_dir_node = Node::default();

        // Generate a secret for the new directory
        let secret = Secret::generate();

        // Store the node in blobs
        let dir_link = Self::_put_node_in_blobs(&new_dir_node, &secret, &self.1).await?;

        // Create a NodeLink for the directory
        let node_link = NodeLink::new_dir(dir_link.clone(), secret);

        // Convert path back to absolute for _set_node_link_at_path
        let abs_path = Path::new("/").join(&path);

        // Use _set_node_link_at_path to insert the directory into the tree
        let (updated_link, node_hashes) =
            Self::_set_node_link_at_path(entry, node_link, &abs_path, &self.1).await?;

        // Update entry if the root was modified
        let new_entry = if let NodeLink::Dir(new_root_link, new_secret) = updated_link {
            Some(
                Self::_get_node_from_blobs(
                    &NodeLink::Dir(new_root_link.clone(), new_secret),
                    &self.1,
                )
                .await?,
            )
        } else {
            None
        };

        // Update inner state
        {
            let mut inner = self.0.lock().await;
            // Track the directory node hash and all created node hashes
            inner.pins.insert(dir_link.hash());
            inner.pins.extend(node_hashes);

            if let Some(new_entry) = new_entry {
                inner.entry = new_entry;
            }

            // Record the mkdir operation in the ops log
            let peer_id = inner.peer_id;
            inner
                .ops_log
                .record(peer_id, OpType::Mkdir, path.to_path_buf(), None, true);
        }

        Ok(())
    }

    /// Move or rename a file or directory from one path to another.
    ///
    /// This operation:
    /// 1. Validates that the move is legal (destination not inside source)
    /// 2. Retrieves the node at the source path
    /// 3. Removes it from the source location
    /// 4. Inserts it at the destination location
    ///
    /// The node's content (files/subdirectories) is not re-encrypted during the move;
    /// only the tree structure is updated. This makes moves efficient regardless of
    /// the size of the subtree being moved.
    ///
    /// # Errors
    ///
    /// - `PathNotFound` - source path doesn't exist
    /// - `PathAlreadyExists` - destination path already exists
    /// - `MoveIntoSelf` - attempting to move a directory into itself (e.g., /foo -> /foo/bar)
    /// - `Default` - attempting to move the root directory
    pub async fn mv(&mut self, from: &Path, to: &Path) -> Result<(), MountError> {
        // Convert absolute paths to relative paths for internal operations.
        // The mount stores paths relative to root, so "/foo/bar" becomes "foo/bar".
        let from_clean = clean_path(from);
        let to_clean = clean_path(to);

        // ============================================================
        // VALIDATION: Prevent moving a directory into itself
        // ============================================================
        // This catches cases like:
        //   - /foo -> /foo (same path, would delete then fail to insert)
        //   - /foo -> /foo/bar (moving into subdirectory of itself)
        //
        // This is impossible in a filesystem sense - you can't put a box inside itself.
        // We check this early to provide a clear error message and avoid corrupting
        // the tree structure (the delete would succeed but the insert would fail).
        if to.starts_with(from) {
            return Err(MountError::MoveIntoSelf {
                from: from.to_path_buf(),
                to: to.to_path_buf(),
            });
        }

        // ============================================================
        // STEP 1: Retrieve the NodeLink at the source path
        // ============================================================
        // A NodeLink is a reference to either a file or directory. For directories,
        // it contains the entire subtree. We'll reuse this same NodeLink at the
        // destination, which means no re-encryption is needed for the content.
        let node_link = self.get(from).await?;
        let is_dir = node_link.is_dir();

        // Store paths for ops log before any mutations
        let from_path = from_clean.to_path_buf();
        let to_path = to_clean.to_path_buf();

        // ============================================================
        // STEP 2: Verify destination doesn't already exist
        // ============================================================
        // Unlike Unix mv which can overwrite, we require the destination to be empty.
        // This prevents accidental data loss.
        if self.get(to).await.is_ok() {
            return Err(MountError::PathAlreadyExists(to.to_path_buf()));
        }

        // ============================================================
        // STEP 3: Remove the node from its source location
        // ============================================================
        // We need to update the parent directory to remove the reference to this node.
        // This involves:
        //   a) Finding the parent directory
        //   b) Removing the child entry from it
        //   c) Re-encrypting and saving the modified parent
        //   d) Propagating changes up to the root (updating all ancestor nodes)
        {
            // Get the parent path (e.g., "foo" for "foo/bar")
            let parent_path = from_clean
                .parent()
                .ok_or_else(|| MountError::Default(anyhow::anyhow!("Cannot move root")))?;

            // Get the current root entry node
            let entry = {
                let inner = self.0.lock().await;
                inner.entry.clone()
            };

            // Load the parent node - either the root itself or a subdirectory
            let mut parent_node = if parent_path == Path::new("") {
                // Source is at root level (e.g., "/foo"), parent is root
                entry.clone()
            } else {
                // Source is nested, need to traverse to find parent
                Self::_get_node_at_path(&entry, parent_path, &self.1).await?
            };

            // Extract the filename component (e.g., "bar" from "foo/bar")
            let file_name = from_clean
                .file_name()
                .expect(
                    "from_clean has no filename - this should be impossible after parent() check",
                )
                .to_string_lossy()
                .to_string();

            // Remove the child from the parent's children map
            if parent_node.del(&file_name).is_none() {
                return Err(MountError::PathNotFound(from_clean.to_path_buf()));
            }

            // Now we need to persist the modified parent and update the tree
            if parent_path == Path::new("") {
                // Parent is root - just update root directly
                let secret = Secret::generate();
                let link = Self::_put_node_in_blobs(&parent_node, &secret, &self.1).await?;

                let mut inner = self.0.lock().await;
                inner.pins.insert(link.hash());
                inner.entry = parent_node;
            } else {
                // Parent is a subdirectory - need to propagate changes up the tree.
                // This creates a new encrypted blob for the parent and updates
                // all ancestor nodes to point to the new parent.
                let secret = Secret::generate();
                let parent_link = Self::_put_node_in_blobs(&parent_node, &secret, &self.1).await?;
                let new_node_link = NodeLink::new_dir(parent_link.clone(), secret);

                // Update the tree from root down to this parent
                let abs_parent_path = Path::new("/").join(parent_path);
                let (updated_root_link, node_hashes) =
                    Self::_set_node_link_at_path(entry, new_node_link, &abs_parent_path, &self.1)
                        .await?;

                // Load the new root entry from the updated link.
                // The root should always be a directory; if it's not, something is
                // seriously wrong with the mount structure.
                let new_entry = Self::_get_node_from_blobs(&updated_root_link, &self.1).await?;

                // Update the mount's internal state with the new tree
                let mut inner = self.0.lock().await;
                inner.pins.insert(parent_link.hash());
                inner.pins.extend(node_hashes);
                inner.entry = new_entry;
            }
        }

        // ============================================================
        // STEP 4: Insert the node at the destination path
        // ============================================================
        // We reuse the same NodeLink from step 1. This means the actual file/directory
        // content doesn't need to be re-encrypted - only the tree structure changes.
        // This makes moves O(depth) rather than O(size of subtree).
        let entry = {
            let inner = self.0.lock().await;
            inner.entry.clone()
        };

        let (updated_root_link, node_hashes) =
            Self::_set_node_link_at_path(entry, node_link, to, &self.1).await?;

        // ============================================================
        // STEP 5: Update internal state with the final tree
        // ============================================================
        {
            // Load the new root entry and update the mount
            let new_entry = Self::_get_node_from_blobs(&updated_root_link, &self.1).await?;

            let mut inner = self.0.lock().await;
            inner.pins.extend(node_hashes);
            inner.entry = new_entry;

            // ============================================================
            // STEP 6: Record mv operation in the ops log
            // ============================================================
            let peer_id = inner.peer_id;
            inner.ops_log.record(
                peer_id,
                OpType::Mv { from: from_path },
                to_path,
                None,
                is_dir,
            );
        }

        Ok(())
    }

    pub async fn ls(&self, path: &Path) -> Result<BTreeMap<PathBuf, NodeLink>, MountError> {
        let mut items = BTreeMap::new();
        let path = clean_path(path);

        let inner = self.0.lock().await;
        let root_node = inner.entry.clone();
        drop(inner);

        let node = if path == Path::new("") {
            root_node
        } else {
            match Self::_get_node_at_path(&root_node, &path, &self.1).await {
                Ok(node) => node,
                Err(MountError::LinkNotFound(_)) => {
                    return Err(MountError::PathNotNode(path.to_path_buf()))
                }
                Err(err) => return Err(err),
            }
        };

        for (name, link) in node.get_links() {
            let mut full_path = path.clone();
            full_path.push(name);
            items.insert(full_path, link.clone());
        }

        Ok(items)
    }

    pub async fn ls_deep(&self, path: &Path) -> Result<BTreeMap<PathBuf, NodeLink>, MountError> {
        let base_path = clean_path(path);
        self._ls_deep(path, &base_path).await
    }

    async fn _ls_deep(
        &self,
        path: &Path,
        base_path: &Path,
    ) -> Result<BTreeMap<PathBuf, NodeLink>, MountError> {
        let mut all_items = BTreeMap::new();

        // get the initial items at the given path
        let items = self.ls(path).await?;

        for (item_path, link) in items {
            // Make path relative to the base_path
            let relative_path = if base_path == Path::new("") {
                item_path.clone()
            } else {
                item_path
                    .strip_prefix(base_path)
                    .unwrap_or(&item_path)
                    .to_path_buf()
            };
            all_items.insert(relative_path.clone(), link.clone());

            if link.is_dir() {
                // Recurse using the absolute path
                let abs_item_path = Path::new("/").join(&item_path);
                let sub_items = Box::pin(self._ls_deep(&abs_item_path, base_path)).await?;

                // Sub items already have correct relative paths from base_path
                for (sub_path, sub_link) in sub_items {
                    all_items.insert(sub_path, sub_link);
                }
            }
        }

        Ok(all_items)
    }

    #[allow(clippy::await_holding_lock)]
    pub async fn cat(&self, path: &Path) -> Result<Vec<u8>, MountError> {
        let path = clean_path(path);

        let inner = self.0.lock().await;
        let root_node = inner.entry.clone();
        drop(inner);

        let (parent_path, file_name) = if let Some(parent) = path.parent() {
            (
                parent,
                path.file_name().unwrap().to_string_lossy().to_string(),
            )
        } else {
            return Err(MountError::PathNotFound(path.to_path_buf()));
        };

        let parent_node = if parent_path == Path::new("") {
            root_node
        } else {
            Self::_get_node_at_path(&root_node, parent_path, &self.1).await?
        };

        let link = parent_node
            .get_link(&file_name)
            .ok_or_else(|| MountError::PathNotFound(path.to_path_buf()))?;

        match link {
            NodeLink::Data(link, secret, _) => {
                let encrypted_data = self.1.get(&link.hash()).await?;
                let data = secret.decrypt(&encrypted_data)?;
                Ok(data)
            }
            NodeLink::Dir(_, _) => Err(MountError::PathNotNode(path.to_path_buf())),
        }
    }

    /// Get the NodeLink for a file at a given path
    #[allow(clippy::await_holding_lock)]
    pub async fn get(&self, path: &Path) -> Result<NodeLink, MountError> {
        let path = clean_path(path);

        let inner = self.0.lock().await;
        let root_node = inner.entry.clone();
        drop(inner);

        let (parent_path, file_name) = if let Some(parent) = path.parent() {
            (
                parent,
                path.file_name().unwrap().to_string_lossy().to_string(),
            )
        } else {
            return Err(MountError::PathNotFound(path.to_path_buf()));
        };

        let parent_node = if parent_path == Path::new("") {
            root_node
        } else {
            Self::_get_node_at_path(&root_node, parent_path, &self.1).await?
        };

        parent_node
            .get_link(&file_name)
            .cloned()
            .ok_or_else(|| MountError::PathNotFound(path.to_path_buf()))
    }

    async fn _get_node_at_path(
        node: &Node,
        path: &Path,
        blobs: &BlobsStore,
    ) -> Result<Node, MountError> {
        let mut current_node = node.clone();
        let mut consumed_path = PathBuf::from("/");

        for part in path.iter() {
            consumed_path.push(part);
            let next = part.to_string_lossy().to_string();
            let next_link = current_node
                .get_link(&next)
                .ok_or(MountError::PathNotFound(consumed_path.clone()))?;
            current_node = Self::_get_node_from_blobs(next_link, blobs).await?
        }
        Ok(current_node)
    }

    pub async fn _set_node_link_at_path(
        node: Node,
        node_link: NodeLink,
        path: &Path,
        blobs: &BlobsStore,
    ) -> Result<(NodeLink, Vec<crate::linked_data::Hash>), MountError> {
        let path = clean_path(path);
        let mut visited_nodes = Vec::new();
        let mut name = path.file_name().unwrap().to_string_lossy().to_string();
        let parent_path = path.parent().unwrap_or(Path::new(""));

        let mut consumed_path = PathBuf::from("/");
        let mut node = node;
        visited_nodes.push((consumed_path.clone(), node.clone()));

        for part in parent_path.iter() {
            let next = part.to_string_lossy().to_string();
            let next_link = node.get_link(&next);
            if let Some(next_link) = next_link {
                consumed_path.push(part);
                match next_link {
                    NodeLink::Dir(..) => {
                        node = Self::_get_node_from_blobs(next_link, blobs).await?
                    }
                    NodeLink::Data(..) => {
                        return Err(MountError::PathNotNode(consumed_path.clone()));
                    }
                }
                visited_nodes.push((consumed_path.clone(), node.clone()));
            } else {
                // Create a new directory node
                node = Node::default();
                consumed_path.push(part);
                visited_nodes.push((consumed_path.clone(), node.clone()));
            }
        }

        let mut node_link = node_link;
        let mut created_hashes = Vec::new();
        for (path, mut node) in visited_nodes.into_iter().rev() {
            node.insert(name, node_link.clone());
            let secret = Secret::generate();
            let link = Self::_put_node_in_blobs(&node, &secret, blobs).await?;
            created_hashes.push(link.hash());
            node_link = NodeLink::Dir(link, secret);
            name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
        }

        Ok((node_link, created_hashes))
    }

    async fn _get_manifest_from_blobs(
        link: &Link,
        blobs: &BlobsStore,
    ) -> Result<Manifest, MountError> {
        tracing::debug!(
            "_get_bucket_from_blobs: Checking for bucket data at link {:?}",
            link
        );
        let hash = link.hash();
        tracing::debug!("_get_bucket_from_blobs: Bucket hash: {}", hash);

        match blobs.stat(&hash).await {
            Ok(true) => {
                tracing::debug!(
                    "_get_bucket_from_blobs: Bucket hash {} exists in blobs",
                    hash
                );
            }
            Ok(false) => {
                tracing::error!("_get_bucket_from_blobs: Bucket hash {} NOT FOUND in blobs - LinkNotFound error!", hash);
                return Err(MountError::LinkNotFound(link.clone()));
            }
            Err(e) => {
                tracing::error!(
                    "_get_bucket_from_blobs: Error checking bucket hash {}: {}",
                    hash,
                    e
                );
                return Err(e.into());
            }
        }

        tracing::debug!("_get_bucket_from_blobs: Reading bucket data from blobs");
        let data = blobs.get(&hash).await?;
        tracing::debug!(
            "_get_bucket_from_blobs: Got {} bytes of bucket data",
            data.len()
        );

        let bucket_data = Manifest::decode(&data)?;
        tracing::debug!(
            "_get_bucket_from_blobs: Successfully decoded BucketData for bucket '{}'",
            bucket_data.name()
        );

        Ok(bucket_data)
    }

    pub async fn _get_pins_from_blobs(link: &Link, blobs: &BlobsStore) -> Result<Pins, MountError> {
        tracing::debug!("_get_pins_from_blobs: Checking for pins at link {:?}", link);
        let hash = link.hash();
        tracing::debug!("_get_pins_from_blobs: Pins hash: {}", hash);

        match blobs.stat(&hash).await {
            Ok(true) => {
                tracing::debug!("_get_pins_from_blobs: Pins hash {} exists in blobs", hash);
            }
            Ok(false) => {
                tracing::error!(
                    "_get_pins_from_blobs: Pins hash {} NOT FOUND in blobs - LinkNotFound error!",
                    hash
                );
                return Err(MountError::LinkNotFound(link.clone()));
            }
            Err(e) => {
                tracing::error!(
                    "_get_pins_from_blobs: Error checking pins hash {}: {}",
                    hash,
                    e
                );
                return Err(e.into());
            }
        }

        tracing::debug!("_get_pins_from_blobs: Reading hash list from blobs");
        // Read hashes from the hash list blob
        let hashes = blobs.read_hash_list(hash).await?;
        tracing::debug!(
            "_get_pins_from_blobs: Successfully read {} hashes from pinset",
            hashes.len()
        );

        Ok(Pins::from_vec(hashes))
    }

    async fn _get_node_from_blobs(
        node_link: &NodeLink,
        blobs: &BlobsStore,
    ) -> Result<Node, MountError> {
        let link = node_link.link();
        let secret = node_link.secret();
        let hash = link.hash();

        tracing::debug!("_get_node_from_blobs: Checking for node at hash {}", hash);

        match blobs.stat(&hash).await {
            Ok(true) => {
                tracing::debug!("_get_node_from_blobs: Node hash {} exists in blobs", hash);
            }
            Ok(false) => {
                tracing::error!(
                    "_get_node_from_blobs: Node hash {} NOT FOUND in blobs - LinkNotFound error!",
                    hash
                );
                return Err(MountError::LinkNotFound(link.clone()));
            }
            Err(e) => {
                tracing::error!(
                    "_get_node_from_blobs: Error checking node hash {}: {}",
                    hash,
                    e
                );
                return Err(e.into());
            }
        }

        tracing::debug!("_get_node_from_blobs: Reading encrypted node blob");
        let blob = blobs.get(&hash).await?;
        tracing::debug!(
            "_get_node_from_blobs: Got {} bytes of encrypted node data",
            blob.len()
        );

        tracing::debug!("_get_node_from_blobs: Decrypting node data");
        let data = secret.decrypt(&blob)?;
        tracing::debug!("_get_node_from_blobs: Decrypted {} bytes", data.len());

        let node = Node::decode(&data)?;
        tracing::debug!("_get_node_from_blobs: Successfully decoded Node");

        Ok(node)
    }

    // TODO (amiller68): you should inline a Link
    //  into the node when we store encrypt it,
    //  so that we have an integrity check
    async fn _put_node_in_blobs(
        node: &Node,
        secret: &Secret,
        blobs: &BlobsStore,
    ) -> Result<Link, MountError> {
        let _data = node.encode()?;
        let data = secret.encrypt(&_data)?;
        let hash = blobs.put(data).await?;
        // NOTE (amiller68): nodes are always stored as raw
        //  since they are encrypted blobs
        let link = Link::new(crate::linked_data::LD_RAW_CODEC, hash);
        Ok(link)
    }

    pub async fn _put_manifest_in_blobs(
        bucket_data: &Manifest,
        blobs: &BlobsStore,
    ) -> Result<Link, MountError> {
        let data = bucket_data.encode()?;
        let hash = blobs.put(data).await?;
        // NOTE (amiller68): buckets are unencrypted, so they can inherit
        //  the codec of the bucket itself (which is currently always cbor)
        let link = Link::new(bucket_data.codec(), hash);
        Ok(link)
    }

    pub async fn _put_pins_in_blobs(pins: &Pins, blobs: &BlobsStore) -> Result<Link, MountError> {
        // Create a hash list blob from the pins (raw bytes: 32 bytes per hash, concatenated)
        let hash = blobs.create_hash_list(pins.iter().copied()).await?;
        // Pins are stored as raw blobs containing concatenated hashes
        // Note: The underlying blob is HashSeq format, but Link doesn't track this
        let link = Link::new(crate::linked_data::LD_RAW_CODEC, hash);
        Ok(link)
    }

    async fn _get_ops_log_from_blobs(
        link: &Link,
        secret: &Secret,
        blobs: &BlobsStore,
    ) -> Result<PathOpLog, MountError> {
        let hash = link.hash();
        tracing::debug!(
            "_get_ops_log_from_blobs: Checking for ops log at hash {}",
            hash
        );

        match blobs.stat(&hash).await {
            Ok(true) => {
                tracing::debug!(
                    "_get_ops_log_from_blobs: Ops log hash {} exists in blobs",
                    hash
                );
            }
            Ok(false) => {
                tracing::error!(
                    "_get_ops_log_from_blobs: Ops log hash {} NOT FOUND in blobs - LinkNotFound error!",
                    hash
                );
                return Err(MountError::LinkNotFound(link.clone()));
            }
            Err(e) => {
                tracing::error!(
                    "_get_ops_log_from_blobs: Error checking ops log hash {}: {}",
                    hash,
                    e
                );
                return Err(e.into());
            }
        }

        tracing::debug!("_get_ops_log_from_blobs: Reading encrypted ops log blob");
        let blob = blobs.get(&hash).await?;
        tracing::debug!(
            "_get_ops_log_from_blobs: Got {} bytes of encrypted ops log data",
            blob.len()
        );

        tracing::debug!("_get_ops_log_from_blobs: Decrypting ops log data");
        let data = secret.decrypt(&blob)?;
        tracing::debug!("_get_ops_log_from_blobs: Decrypted {} bytes", data.len());

        let ops_log = PathOpLog::decode(&data)?;
        tracing::debug!(
            "_get_ops_log_from_blobs: Successfully decoded PathOpLog with {} operations",
            ops_log.len()
        );

        Ok(ops_log)
    }

    async fn _put_ops_log_in_blobs(
        ops_log: &PathOpLog,
        secret: &Secret,
        blobs: &BlobsStore,
    ) -> Result<Link, MountError> {
        let _data = ops_log.encode()?;
        let data = secret.encrypt(&_data)?;
        let hash = blobs.put(data).await?;
        // Ops log is stored as an encrypted raw blob
        let link = Link::new(crate::linked_data::LD_RAW_CODEC, hash);
        tracing::debug!(
            "_put_ops_log_in_blobs: Stored ops log with {} operations at hash {}",
            ops_log.len(),
            hash
        );
        Ok(link)
    }

    /// Collect all ops from manifest chain back to (but not including) ancestor_link.
    ///
    /// Traverses the manifest chain starting from the current version, collecting
    /// ops_logs from each manifest until we reach the ancestor (or genesis).
    /// The collected ops are merged in chronological order.
    ///
    /// # Arguments
    ///
    /// * `ancestor_link` - Stop when reaching this link (not included). None means collect all
    ///   accessible ops (stops when we can't decrypt a manifest).
    /// * `blobs` - The blob store to read manifests from
    ///
    /// # Returns
    ///
    /// A combined PathOpLog containing all operations since the ancestor.
    pub async fn collect_ops_since(
        &self,
        ancestor_link: Option<&Link>,
        blobs: &BlobsStore,
    ) -> Result<PathOpLog, MountError> {
        let inner = self.0.lock().await;
        let secret_key = inner.secret_key.clone();
        let current_link = inner.link.clone();
        let current_ops = inner.ops_log.clone();
        drop(inner);

        let mut all_logs: Vec<PathOpLog> = Vec::new();

        // Start with any unsaved operations in the current mount
        if !current_ops.is_empty() {
            all_logs.push(current_ops);
        }

        // Walk the chain from current manifest backwards
        let mut link = current_link;

        loop {
            // Check if we've reached the ancestor
            if let Some(ancestor) = ancestor_link {
                if &link == ancestor {
                    break;
                }
            }

            // Load the manifest at this link
            let manifest = Self::_get_manifest_from_blobs(&link, blobs).await?;

            // Get the secret for this manifest version
            // If we can't get the secret (e.g., we weren't a member at this point),
            // stop traversing - we can't read older ops anyway
            let secret = match self.get_secret_for_manifest(&manifest, &secret_key) {
                Ok(s) => s,
                Err(MountError::ShareNotFound) => {
                    tracing::debug!(
                        "collect_ops_since: stopping at link {} - no share for current user",
                        link.hash()
                    );
                    break;
                }
                Err(e) => return Err(e),
            };

            // Load the ops_log if present
            if let Some(ops_link) = manifest.ops_log() {
                let mut ops_log = Self::_get_ops_log_from_blobs(ops_link, &secret, blobs).await?;
                ops_log.rebuild_clock();
                all_logs.push(ops_log);
            }

            // Move to previous manifest
            match manifest.previous() {
                Some(prev) => link = prev.clone(),
                None => break, // Reached genesis
            }
        }

        // Merge all logs in chronological order (oldest first, so reverse)
        all_logs.reverse();
        let mut merged = PathOpLog::new();
        for log in all_logs {
            merged.merge(&log);
        }

        Ok(merged)
    }

    /// Get the decryption secret for a manifest.
    ///
    /// Decrypts the secret share using the provided secret key.
    #[allow(clippy::result_large_err)]
    fn get_secret_for_manifest(
        &self,
        manifest: &Manifest,
        secret_key: &SecretKey,
    ) -> Result<Secret, MountError> {
        let public_key = secret_key.public();
        let share = manifest
            .get_share(&public_key)
            .ok_or(MountError::ShareNotFound)?;

        match share.role() {
            PrincipalRole::Owner => {
                let secret_share = share.share().ok_or(MountError::ShareNotFound)?;
                Ok(secret_share.recover(secret_key)?)
            }
            PrincipalRole::Mirror => manifest
                .public()
                .cloned()
                .ok_or(MountError::MirrorCannotMount),
        }
    }

    /// Find the common ancestor between this mount's chain and another's.
    ///
    /// Walks both chains backwards via the `previous` links and returns
    /// the first link where both chains converge.
    ///
    /// # Arguments
    ///
    /// * `other` - The other mount to find common ancestor with
    /// * `blobs` - The blob store to read manifests from
    ///
    /// # Returns
    ///
    /// The common ancestor link, or None if chains never converge (different buckets).
    pub async fn find_common_ancestor(
        &self,
        other: &Mount,
        blobs: &BlobsStore,
    ) -> Result<Option<Link>, MountError> {
        // Build set of all links in self's chain
        let mut self_chain: std::collections::HashSet<Link> = std::collections::HashSet::new();

        let self_link = self.link().await;
        let mut link = self_link;

        loop {
            self_chain.insert(link.clone());
            let manifest = Self::_get_manifest_from_blobs(&link, blobs).await?;
            match manifest.previous() {
                Some(prev) => link = prev.clone(),
                None => break,
            }
        }

        // Walk other's chain and find first link in self's set
        let other_link = other.link().await;
        let mut link = other_link;

        loop {
            if self_chain.contains(&link) {
                return Ok(Some(link));
            }
            let manifest = Self::_get_manifest_from_blobs(&link, blobs).await?;
            match manifest.previous() {
                Some(prev) => link = prev.clone(),
                None => break,
            }
        }

        // No common ancestor found
        Ok(None)
    }

    /// Apply resolved operations to the entry tree.
    ///
    /// For each operation in the resolved state:
    /// - Files with content links are checked for existence (cannot recreate without secret)
    /// - Directories are created if they don't exist
    async fn apply_resolved_state(&mut self, merged_ops: &PathOpLog) -> Result<(), MountError> {
        let resolved_state = merged_ops.resolve_all();

        for (path, op) in &resolved_state {
            if op.content_link.is_some() {
                // File operation - check if it exists in our tree
                let abs_path = Path::new("/").join(path);
                match self.get(&abs_path).await {
                    Ok(_) => {
                        // File exists, skip
                    }
                    Err(MountError::PathNotFound(_)) => {
                        // Can't recreate - ops_log stores Link but not decryption secret
                        tracing::warn!(
                            "apply_resolved_state: cannot recreate file {} - no secret",
                            path.display()
                        );
                        // Record in ops_log so it's tracked
                        let mut inner = self.0.lock().await;
                        inner.ops_log.merge(&PathOpLog::from_operation(op));
                    }
                    Err(e) => return Err(e),
                }
            } else if op.is_dir && matches!(op.op_type, super::path_ops::OpType::Mkdir) {
                // Directory operation - create if missing
                let abs_path = Path::new("/").join(path);
                match self.mkdir(&abs_path).await {
                    Ok(()) => {}
                    Err(MountError::PathAlreadyExists(_)) => {}
                    Err(e) => return Err(e),
                }
            }
        }

        Ok(())
    }

    /// Merge another mount's changes into this one using the given resolver.
    ///
    /// This method:
    /// 1. Finds the common ancestor between the two chains
    /// 2. Collects ops from both chains since that ancestor
    /// 3. Merges using the resolver
    /// 4. Applies the merged state
    /// 5. Saves the result as a new version
    ///
    /// # Arguments
    ///
    /// * `incoming` - The mount to merge changes from
    /// * `resolver` - The conflict resolution strategy to use
    /// * `blobs` - The blob store
    ///
    /// # Returns
    ///
    /// A MergeResult containing conflict information and the new version link.
    pub async fn merge_from<R: super::ConflictResolver>(
        &mut self,
        incoming: &Mount,
        resolver: &R,
        blobs: &BlobsStore,
    ) -> Result<(MergeResult, Link), MountError> {
        // Find the common ancestor
        let ancestor = self.find_common_ancestor(incoming, blobs).await?;

        // Collect ops from both chains since the ancestor
        let local_ops = self.collect_ops_since(ancestor.as_ref(), blobs).await?;
        let incoming_ops = incoming.collect_ops_since(ancestor.as_ref(), blobs).await?;

        // Get local peer ID for tie-breaking
        let peer_id = {
            let inner = self.0.lock().await;
            inner.peer_id
        };

        // Merge the operations
        let mut merged_ops = local_ops.clone();
        let merge_result = merged_ops.merge_with_resolver(&incoming_ops, resolver, &peer_id);

        // Apply merged state to the entry tree
        self.apply_resolved_state(&merged_ops).await?;

        // Merge the ops_log to include all merged operations
        {
            let mut inner = self.0.lock().await;
            inner.ops_log.merge(&merged_ops);
        }

        // Save the merged state
        let (link, _, _) = self.save(blobs, false).await?;

        Ok((merge_result, link))
    }
}
