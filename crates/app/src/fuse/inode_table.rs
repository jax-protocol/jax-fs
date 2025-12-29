//! Inode to path mapping for FUSE filesystem
//!
//! FUSE uses inodes (u64) to identify files, but our Mount uses paths.
//! This module provides bidirectional mapping between them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Bidirectional mapping between inodes and paths
pub struct InodeTable {
    path_to_inode: HashMap<PathBuf, u64>,
    inode_to_path: HashMap<u64, PathBuf>,
    next_inode: u64,
}

impl InodeTable {
    /// Create a new inode table with root directory at inode 1
    pub fn new() -> Self {
        let mut table = Self {
            path_to_inode: HashMap::new(),
            inode_to_path: HashMap::new(),
            next_inode: 2, // Start at 2, 1 is reserved for root
        };

        // Root directory is always inode 1
        let root = PathBuf::from("/");
        table.path_to_inode.insert(root.clone(), 1);
        table.inode_to_path.insert(1, root);

        table
    }

    /// Get inode for a path, creating one if it doesn't exist
    pub fn get_or_create(&mut self, path: &Path) -> u64 {
        if let Some(&ino) = self.path_to_inode.get(path) {
            return ino;
        }

        let ino = self.next_inode;
        self.next_inode += 1;
        self.path_to_inode.insert(path.to_path_buf(), ino);
        self.inode_to_path.insert(ino, path.to_path_buf());
        ino
    }

    /// Get inode for a path if it exists
    pub fn get_inode(&self, path: &Path) -> Option<u64> {
        self.path_to_inode.get(path).copied()
    }

    /// Get path for an inode
    pub fn get_path(&self, inode: u64) -> Option<&Path> {
        self.inode_to_path.get(&inode).map(|p| p.as_path())
    }

    /// Remove an inode mapping (for deleted files)
    pub fn remove(&mut self, inode: u64) {
        if let Some(path) = self.inode_to_path.remove(&inode) {
            self.path_to_inode.remove(&path);
        }
    }

    /// Rename a path (for mv operations)
    pub fn rename(&mut self, from: &Path, to: &Path) {
        if let Some(ino) = self.path_to_inode.remove(from) {
            self.path_to_inode.insert(to.to_path_buf(), ino);
            self.inode_to_path.insert(ino, to.to_path_buf());
        }
    }
}

impl Default for InodeTable {
    fn default() -> Self {
        Self::new()
    }
}
