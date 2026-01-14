use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Mutex;
use uuid::Uuid;

use crate::crypto::{PublicKey, Secret, SecretError, SecretKey, SecretShare};
use crate::linked_data::{BlockEncoded, CodecError, Link};
use crate::peer::{BlobsStore, BlobsStoreError};

use super::manifest::Manifest;
use super::node::{Node, NodeError, NodeLink};
use super::path_ops::{OpType, PathOpLog};
use super::pins::Pins;

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
    #[error("peers share was not found. this should be impossible")]
    ShareNotFound,
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

    /// Save the current mount state to the blobs store
    pub async fn save(&self, blobs: &BlobsStore) -> Result<(Link, Link, u64), MountError> {
        // Clone data we need before any async operations
        let (entry_node, mut pins, previous_link, previous_height, manifest_template, ops_log) = {
            let inner = self.0.lock().await;
            (
                inner.entry.clone(),
                inner.pins.clone(),
                inner.link.clone(),
                inner.height,
                inner.manifest.clone(),
                inner.ops_log.clone(),
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

        // Update the bucket's share with the new root link
        // (add_share creates the Share internally)
        let mut manifest = manifest_template;
        let _m = manifest.clone();
        let shares = _m.shares();
        manifest.unset_shares();
        for share in shares.values() {
            let public_key = share.principal().identity;
            manifest.add_share(public_key, secret.clone())?;
        }
        manifest.set_pins(pins_link.clone());
        manifest.set_previous(previous_link.clone());
        manifest.set_entry(entry.clone());
        manifest.set_height(height);

        // Set the ops log link if we have operations
        if let Some(ops_link) = ops_log_link {
            manifest.set_ops_log(ops_link);
        }

        // Put the updated manifest into blobs to determine the new link
        let link = Self::_put_manifest_in_blobs(&manifest, blobs).await?;

        // Update the internal state
        {
            let mut inner = self.0.lock().await;
            inner.manifest = manifest;
            inner.height = height;
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
        let manifest = Manifest::new(
            id,
            name.clone(),
            owner.public(),
            share,
            entry_link.clone(),
            pins_link.clone(),
            0, // initial height is 0
        );
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

        let _share = manifest.get_share(public_key);

        let share = match _share {
            Some(share) => share.share(),
            None => return Err(MountError::ShareNotFound),
        };

        let secret = share.recover(secret_key)?;

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
            })),
            blobs.clone(),
        ))
    }

    pub async fn share(&mut self, peer: PublicKey) -> Result<(), MountError> {
        let mut inner = self.0.lock().await;
        inner.manifest.add_share(peer, Secret::default())?;
        Ok(())
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
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Cursor;
    use tempfile::TempDir;

    async fn setup_test_env() -> (Mount, BlobsStore, crate::crypto::SecretKey, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let blob_path = temp_dir.path().join("blobs");

        let secret_key = SecretKey::generate();
        // Generate iroh secret key from random bytes
        let blobs = BlobsStore::fs(&blob_path).await.unwrap();

        let mount = Mount::init(Uuid::new_v4(), "test".to_string(), &secret_key, &blobs)
            .await
            .unwrap();

        (mount, blobs, secret_key, temp_dir)
    }

    #[tokio::test]
    async fn test_add_and_cat() {
        let (mut mount, _, _, _temp) = setup_test_env().await;

        let data = b"Hello, world!";
        let path = PathBuf::from("/test.txt");

        mount.add(&path, Cursor::new(data.to_vec())).await.unwrap();

        let result = mount.cat(&path).await.unwrap();
        assert_eq!(result, data);
    }

    #[tokio::test]
    async fn test_add_with_metadata() {
        let (mut mount, _blobs, _, _temp) = setup_test_env().await;

        let data = b"{ \"key\": \"value\" }";
        let path = PathBuf::from("/data.json");

        mount.add(&path, Cursor::new(data.to_vec())).await.unwrap();

        let items = mount.ls(&PathBuf::from("/")).await.unwrap();
        assert_eq!(items.len(), 1);

        let (file_path, link) = items.iter().next().unwrap();
        assert_eq!(file_path, &PathBuf::from("data.json"));

        if let Some(data_info) = link.data() {
            assert!(data_info.mime().is_some());
            assert_eq!(data_info.mime().unwrap().as_ref(), "application/json");
        } else {
            panic!("Expected data link with metadata");
        }
    }

    #[tokio::test]
    async fn test_ls() {
        let (mut mount, _blobs, _, _temp) = setup_test_env().await;

        mount
            .add(&PathBuf::from("/file1.txt"), Cursor::new(b"data1".to_vec()))
            .await
            .unwrap();
        mount
            .add(&PathBuf::from("/file2.txt"), Cursor::new(b"data2".to_vec()))
            .await
            .unwrap();
        mount
            .add(
                &PathBuf::from("/dir/file3.txt"),
                Cursor::new(b"data3".to_vec()),
            )
            .await
            .unwrap();

        let items = mount.ls(&PathBuf::from("/")).await.unwrap();
        assert_eq!(items.len(), 3);

        assert!(items.contains_key(&PathBuf::from("file1.txt")));
        assert!(items.contains_key(&PathBuf::from("file2.txt")));
        assert!(items.contains_key(&PathBuf::from("dir")));

        let sub_items = mount.ls(&PathBuf::from("/dir")).await.unwrap();
        assert_eq!(sub_items.len(), 1);
        assert!(sub_items.contains_key(&PathBuf::from("dir/file3.txt")));
    }

    #[tokio::test]
    async fn test_ls_deep() {
        let (mut mount, _blobs, _, _temp) = setup_test_env().await;

        mount
            .add(&PathBuf::from("/a.txt"), Cursor::new(b"a".to_vec()))
            .await
            .unwrap();
        mount
            .add(&PathBuf::from("/dir1/b.txt"), Cursor::new(b"b".to_vec()))
            .await
            .unwrap();
        mount
            .add(
                &PathBuf::from("/dir1/dir2/c.txt"),
                Cursor::new(b"c".to_vec()),
            )
            .await
            .unwrap();
        mount
            .add(
                &PathBuf::from("/dir1/dir2/dir3/d.txt"),
                Cursor::new(b"d".to_vec()),
            )
            .await
            .unwrap();

        let all_items = mount.ls_deep(&PathBuf::from("/")).await.unwrap();

        assert!(all_items.contains_key(&PathBuf::from("a.txt")));
        assert!(all_items.contains_key(&PathBuf::from("dir1")));
        assert!(all_items.contains_key(&PathBuf::from("dir1/b.txt")));
        assert!(all_items.contains_key(&PathBuf::from("dir1/dir2")));
        assert!(all_items.contains_key(&PathBuf::from("dir1/dir2/c.txt")));
        assert!(all_items.contains_key(&PathBuf::from("dir1/dir2/dir3")));
        assert!(all_items.contains_key(&PathBuf::from("dir1/dir2/dir3/d.txt")));
    }

    #[tokio::test]
    async fn test_rm() {
        let (mut mount, _blobs, _, _temp) = setup_test_env().await;

        mount
            .add(&PathBuf::from("/file1.txt"), Cursor::new(b"data1".to_vec()))
            .await
            .unwrap();
        mount
            .add(&PathBuf::from("/file2.txt"), Cursor::new(b"data2".to_vec()))
            .await
            .unwrap();

        let items = mount.ls(&PathBuf::from("/")).await.unwrap();
        assert_eq!(items.len(), 2);

        mount.rm(&PathBuf::from("/file1.txt")).await.unwrap();

        let items = mount.ls(&PathBuf::from("/")).await.unwrap();
        assert_eq!(items.len(), 1);
        assert!(items.contains_key(&PathBuf::from("file2.txt")));
        assert!(!items.contains_key(&PathBuf::from("file1.txt")));

        let result = mount.cat(&PathBuf::from("/file1.txt")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_nested_operations() {
        let (mut mount, _blobs, _, _temp) = setup_test_env().await;

        let files = vec![
            ("/root.txt", b"root" as &[u8]),
            ("/docs/readme.md", b"readme" as &[u8]),
            ("/docs/guide.pdf", b"guide" as &[u8]),
            ("/src/main.rs", b"main" as &[u8]),
            ("/src/lib.rs", b"lib" as &[u8]),
            ("/src/tests/unit.rs", b"unit" as &[u8]),
            ("/src/tests/integration.rs", b"integration" as &[u8]),
        ];

        for (path, data) in &files {
            mount
                .add(&PathBuf::from(path), Cursor::new(data.to_vec()))
                .await
                .unwrap();
        }

        for (path, expected_data) in &files {
            let data = mount.cat(&PathBuf::from(path)).await.unwrap();
            assert_eq!(data, expected_data.to_vec());
        }

        mount
            .rm(&PathBuf::from("/src/tests/unit.rs"))
            .await
            .unwrap();

        let result = mount.cat(&PathBuf::from("/src/tests/unit.rs")).await;
        assert!(result.is_err());

        let data = mount
            .cat(&PathBuf::from("/src/tests/integration.rs"))
            .await
            .unwrap();
        assert_eq!(data, b"integration");
    }

    #[tokio::test]
    async fn test_various_file_types() {
        let (mut mount, _blobs, _, _temp) = setup_test_env().await;

        let test_files = vec![
            ("/image.png", "image/png"),
            ("/video.mp4", "video/mp4"),
            ("/style.css", "text/css"),
            ("/script.js", "text/javascript"),
            ("/data.json", "application/json"),
            ("/archive.zip", "application/zip"),
            ("/document.pdf", "application/pdf"),
            ("/code.rs", "text/x-rust"),
        ];

        for (path, expected_mime) in test_files {
            mount
                .add(&PathBuf::from(path), Cursor::new(b"test".to_vec()))
                .await
                .unwrap();

            let items = mount.ls(&PathBuf::from("/")).await.unwrap();
            let link = items.values().find(|l| l.is_data()).unwrap();

            if let Some(data_info) = link.data() {
                assert!(data_info.mime().is_some());
                assert_eq!(data_info.mime().unwrap().as_ref(), expected_mime);
            }

            mount.rm(&PathBuf::from(path)).await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_error_cases() {
        let (mount, _blobs, _, _temp) = setup_test_env().await;

        let result = mount.cat(&PathBuf::from("/does_not_exist.txt")).await;
        assert!(result.is_err());

        let result = mount.ls(&PathBuf::from("/does_not_exist")).await;
        assert!(result.is_err() || result.unwrap().is_empty());

        let (mut mount, _blobs, _, _temp) = setup_test_env().await;
        mount
            .add(
                &PathBuf::from("/dir/file.txt"),
                Cursor::new(b"data".to_vec()),
            )
            .await
            .unwrap();

        let result = mount.cat(&PathBuf::from("/dir")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_save_load() {
        let (mount, blobs, secret_key, _temp) = setup_test_env().await;
        let (link, _previous_link, height) = mount.save(&blobs).await.unwrap();
        assert_eq!(height, 1); // Height should be 1 after first save
        let loaded_mount = Mount::load(&link, &secret_key, &blobs).await.unwrap();
        assert_eq!(loaded_mount.inner().await.height(), 1);
    }

    #[tokio::test]
    async fn test_mkdir() {
        let (mut mount, _blobs, _, _temp) = setup_test_env().await;

        // Create a directory
        mount.mkdir(&PathBuf::from("/test_dir")).await.unwrap();

        // Verify it exists and is a directory
        let items = mount.ls(&PathBuf::from("/")).await.unwrap();
        assert_eq!(items.len(), 1);
        assert!(items.get(&PathBuf::from("test_dir")).unwrap().is_dir());
    }

    #[tokio::test]
    async fn test_mkdir_nested() {
        let (mut mount, _blobs, _, _temp) = setup_test_env().await;

        // Create nested directories (should create parents automatically)
        mount.mkdir(&PathBuf::from("/a/b/c")).await.unwrap();

        // Verify the whole path exists
        let items_root = mount.ls(&PathBuf::from("/")).await.unwrap();
        assert!(items_root.contains_key(&PathBuf::from("a")));

        let items_a = mount.ls(&PathBuf::from("/a")).await.unwrap();
        assert!(items_a.contains_key(&PathBuf::from("a/b")));

        let items_b = mount.ls(&PathBuf::from("/a/b")).await.unwrap();
        assert!(items_b.contains_key(&PathBuf::from("a/b/c")));
        assert!(items_b.get(&PathBuf::from("a/b/c")).unwrap().is_dir());
    }

    #[tokio::test]
    async fn test_mkdir_already_exists() {
        let (mut mount, _blobs, _, _temp) = setup_test_env().await;

        // Create directory
        mount.mkdir(&PathBuf::from("/test_dir")).await.unwrap();

        // Try to create it again - should error
        let result = mount.mkdir(&PathBuf::from("/test_dir")).await;
        assert!(matches!(result, Err(MountError::PathAlreadyExists(_))));
    }

    #[tokio::test]
    async fn test_mkdir_file_exists() {
        let (mut mount, _blobs, _, _temp) = setup_test_env().await;

        // Create a file
        mount
            .add(&PathBuf::from("/test.txt"), Cursor::new(b"data".to_vec()))
            .await
            .unwrap();

        // Try to create directory with same name - should error
        let result = mount.mkdir(&PathBuf::from("/test.txt")).await;
        assert!(matches!(result, Err(MountError::PathAlreadyExists(_))));
    }

    #[tokio::test]
    async fn test_mkdir_then_add_file() {
        let (mut mount, _blobs, _, _temp) = setup_test_env().await;

        // Create a directory
        mount.mkdir(&PathBuf::from("/docs")).await.unwrap();

        // Add a file to the created directory
        mount
            .add(
                &PathBuf::from("/docs/readme.md"),
                Cursor::new(b"# README".to_vec()),
            )
            .await
            .unwrap();

        // Verify the file exists
        let data = mount.cat(&PathBuf::from("/docs/readme.md")).await.unwrap();
        assert_eq!(data, b"# README");

        // Verify directory structure
        let items = mount.ls(&PathBuf::from("/docs")).await.unwrap();
        assert_eq!(items.len(), 1);
        assert!(items.contains_key(&PathBuf::from("docs/readme.md")));
    }

    #[tokio::test]
    async fn test_mkdir_multiple_siblings() {
        let (mut mount, _blobs, _, _temp) = setup_test_env().await;

        // Create multiple sibling directories
        mount.mkdir(&PathBuf::from("/dir1")).await.unwrap();
        mount.mkdir(&PathBuf::from("/dir2")).await.unwrap();
        mount.mkdir(&PathBuf::from("/dir3")).await.unwrap();

        // Verify all exist
        let items = mount.ls(&PathBuf::from("/")).await.unwrap();
        assert_eq!(items.len(), 3);
        assert!(items.get(&PathBuf::from("dir1")).unwrap().is_dir());
        assert!(items.get(&PathBuf::from("dir2")).unwrap().is_dir());
        assert!(items.get(&PathBuf::from("dir3")).unwrap().is_dir());
    }

    #[tokio::test]
    async fn test_mv_file() {
        let (mut mount, _blobs, _, _temp) = setup_test_env().await;

        // Create a file
        mount
            .add(&PathBuf::from("/old.txt"), Cursor::new(b"data".to_vec()))
            .await
            .unwrap();

        // Move the file
        mount
            .mv(&PathBuf::from("/old.txt"), &PathBuf::from("/new.txt"))
            .await
            .unwrap();

        // Verify old path doesn't exist
        let result = mount.cat(&PathBuf::from("/old.txt")).await;
        assert!(result.is_err());

        // Verify new path exists with same content
        let data = mount.cat(&PathBuf::from("/new.txt")).await.unwrap();
        assert_eq!(data, b"data");
    }

    #[tokio::test]
    async fn test_mv_file_to_subdir() {
        let (mut mount, _blobs, _, _temp) = setup_test_env().await;

        // Create a file
        mount
            .add(&PathBuf::from("/file.txt"), Cursor::new(b"data".to_vec()))
            .await
            .unwrap();

        // Move to a new subdirectory (should create it)
        mount
            .mv(
                &PathBuf::from("/file.txt"),
                &PathBuf::from("/subdir/file.txt"),
            )
            .await
            .unwrap();

        // Verify old path doesn't exist
        let result = mount.cat(&PathBuf::from("/file.txt")).await;
        assert!(result.is_err());

        // Verify new path exists
        let data = mount.cat(&PathBuf::from("/subdir/file.txt")).await.unwrap();
        assert_eq!(data, b"data");
    }

    #[tokio::test]
    async fn test_mv_directory() {
        let (mut mount, _blobs, _, _temp) = setup_test_env().await;

        // Create a directory with files
        mount
            .add(
                &PathBuf::from("/olddir/file1.txt"),
                Cursor::new(b"data1".to_vec()),
            )
            .await
            .unwrap();
        mount
            .add(
                &PathBuf::from("/olddir/file2.txt"),
                Cursor::new(b"data2".to_vec()),
            )
            .await
            .unwrap();

        // Move the directory
        mount
            .mv(&PathBuf::from("/olddir"), &PathBuf::from("/newdir"))
            .await
            .unwrap();

        // Verify old directory doesn't exist
        let result = mount.ls(&PathBuf::from("/olddir")).await;
        assert!(result.is_err());

        // Verify new directory exists with files
        let items = mount.ls(&PathBuf::from("/newdir")).await.unwrap();
        assert_eq!(items.len(), 2);

        // Verify file contents
        let data = mount
            .cat(&PathBuf::from("/newdir/file1.txt"))
            .await
            .unwrap();
        assert_eq!(data, b"data1");
    }

    #[tokio::test]
    async fn test_mv_not_found() {
        let (mut mount, _blobs, _, _temp) = setup_test_env().await;

        // Try to move a non-existent file
        let result = mount
            .mv(
                &PathBuf::from("/nonexistent.txt"),
                &PathBuf::from("/new.txt"),
            )
            .await;
        assert!(matches!(result, Err(MountError::PathNotFound(_))));
    }

    #[tokio::test]
    async fn test_mv_already_exists() {
        let (mut mount, _blobs, _, _temp) = setup_test_env().await;

        // Create two files
        mount
            .add(&PathBuf::from("/file1.txt"), Cursor::new(b"data1".to_vec()))
            .await
            .unwrap();
        mount
            .add(&PathBuf::from("/file2.txt"), Cursor::new(b"data2".to_vec()))
            .await
            .unwrap();

        // Try to move file1 to file2 (should fail)
        let result = mount
            .mv(&PathBuf::from("/file1.txt"), &PathBuf::from("/file2.txt"))
            .await;
        assert!(matches!(result, Err(MountError::PathAlreadyExists(_))));
    }

    #[tokio::test]
    async fn test_mv_into_self() {
        let (mut mount, _blobs, _, _temp) = setup_test_env().await;

        // Create a directory with a file inside
        mount.mkdir(&PathBuf::from("/parent")).await.unwrap();
        mount
            .add(
                &PathBuf::from("/parent/child.txt"),
                Cursor::new(b"data".to_vec()),
            )
            .await
            .unwrap();

        // Try to move directory into itself (should fail)
        let result = mount
            .mv(&PathBuf::from("/parent"), &PathBuf::from("/parent/nested"))
            .await;
        assert!(matches!(result, Err(MountError::MoveIntoSelf { .. })));

        // Try to move directory to same path (should also fail)
        let result = mount
            .mv(&PathBuf::from("/parent"), &PathBuf::from("/parent"))
            .await;
        assert!(matches!(result, Err(MountError::MoveIntoSelf { .. })));

        // Verify original directory still exists and is intact
        let items = mount.ls(&PathBuf::from("/parent")).await.unwrap();
        assert_eq!(items.len(), 1);

        // Verify child file still accessible
        let data = mount
            .cat(&PathBuf::from("/parent/child.txt"))
            .await
            .unwrap();
        assert_eq!(data, b"data");
    }

    #[tokio::test]
    async fn test_ops_log_records_operations() {
        let (mut mount, _blobs, _, _temp) = setup_test_env().await;

        // Perform various operations
        mount
            .add(&PathBuf::from("/file1.txt"), Cursor::new(b"data".to_vec()))
            .await
            .unwrap();
        mount.mkdir(&PathBuf::from("/dir")).await.unwrap();
        mount
            .mv(
                &PathBuf::from("/file1.txt"),
                &PathBuf::from("/dir/file1.txt"),
            )
            .await
            .unwrap();
        mount.rm(&PathBuf::from("/dir/file1.txt")).await.unwrap();

        // Verify ops log has recorded all operations
        let inner = mount.inner().await;
        let ops_log = inner.ops_log();

        // Should have 4 operations: Add, Mkdir, Mv, Remove
        assert_eq!(ops_log.len(), 4);

        let ops: Vec<_> = ops_log.ops_in_order().collect();
        assert!(matches!(ops[0].op_type, OpType::Add));
        assert!(matches!(ops[1].op_type, OpType::Mkdir));
        assert!(matches!(ops[2].op_type, OpType::Mv { .. }));
        assert!(matches!(ops[3].op_type, OpType::Remove));
    }

    #[tokio::test]
    async fn test_ops_log_persists_across_save_load() {
        let (mut mount, blobs, secret_key, _temp) = setup_test_env().await;

        // Perform some operations
        mount
            .add(&PathBuf::from("/file.txt"), Cursor::new(b"data".to_vec()))
            .await
            .unwrap();
        mount.mkdir(&PathBuf::from("/dir")).await.unwrap();

        // Save
        let (link, _, _) = mount.save(&blobs).await.unwrap();

        // Load the mount
        let loaded_mount = Mount::load(&link, &secret_key, &blobs).await.unwrap();

        // Verify ops log was loaded
        let inner = loaded_mount.inner().await;
        let ops_log = inner.ops_log();
        assert_eq!(ops_log.len(), 2);

        let ops: Vec<_> = ops_log.ops_in_order().collect();
        assert!(matches!(ops[0].op_type, OpType::Add));
        assert!(matches!(ops[1].op_type, OpType::Mkdir));
    }
}
