//! BAO file storage for object store-backed blob store.
//!
//! This module provides storage for complete and partial blobs backed by object storage.
//! Unlike the filesystem-based FsStore, all data is stored in object storage (S3/MinIO/etc).

use std::fmt;
use std::io;
use std::sync::Arc;

use bao_tree::{io::fsm::BaoContentItem, BaoTree, ChunkRanges};
use bytes::BytesMut;
use iroh_blobs::api::blobs::Bitfield;

use super::entry_state::needs_outboard;
use super::minio::BlobObjectStore;

/// Block size used by iroh (16 KiB)
pub const IROH_BLOCK_SIZE: bao_tree::BlockSize = bao_tree::BlockSize::from_chunk_log(4);

/// Storage for complete blobs in object storage.
#[derive(Debug, Clone)]
pub struct CompleteStorage {
    /// Hash of the blob
    pub hash: String,
    /// Size of the blob in bytes
    pub size: u64,
    /// Whether this blob has outboard data
    pub has_outboard: bool,
}

impl CompleteStorage {
    /// Create a new complete storage reference.
    pub fn new(hash: String, size: u64) -> Self {
        Self {
            hash,
            size,
            has_outboard: needs_outboard(size),
        }
    }

    /// Get the bitfield for this complete blob.
    pub fn bitfield(&self) -> Bitfield {
        Bitfield::complete(self.size)
    }
}

/// Storage for partial (incomplete) blobs.
#[derive(Debug, Clone)]
pub struct PartialStorage {
    /// Hash of the blob
    pub hash: String,
    /// Size of the blob if known
    pub size: Option<u64>,
    /// Bitfield tracking which chunks we have
    pub bitfield: Bitfield,
    /// Buffered data waiting to be flushed
    pub data_buffer: BytesMut,
    /// Buffered outboard waiting to be flushed
    pub outboard_buffer: BytesMut,
}

impl PartialStorage {
    /// Create a new partial storage.
    pub fn new(hash: String, size: Option<u64>) -> Self {
        let bitfield = if let Some(s) = size {
            Bitfield::new(ChunkRanges::empty(), s)
        } else {
            Bitfield::empty()
        };

        Self {
            hash,
            size,
            bitfield,
            data_buffer: BytesMut::new(),
            outboard_buffer: BytesMut::new(),
        }
    }

    /// Write a batch of BAO content items.
    pub fn write_batch(&mut self, size: u64, batch: &[BaoContentItem]) -> io::Result<()> {
        let tree = BaoTree::new(size, IROH_BLOCK_SIZE);

        for item in batch {
            match item {
                BaoContentItem::Parent(parent) => {
                    if let Some(offset) = tree.pre_order_offset(parent.node) {
                        let o0 = (offset * 64) as usize;
                        // Ensure buffer is large enough
                        if self.outboard_buffer.len() < o0 + 64 {
                            self.outboard_buffer.resize(o0 + 64, 0);
                        }
                        self.outboard_buffer[o0..o0 + 32]
                            .copy_from_slice(parent.pair.0.as_bytes());
                        self.outboard_buffer[o0 + 32..o0 + 64]
                            .copy_from_slice(parent.pair.1.as_bytes());
                    }
                }
                BaoContentItem::Leaf(leaf) => {
                    let offset = leaf.offset as usize;
                    let end = offset + leaf.data.len();
                    // Ensure buffer is large enough
                    if self.data_buffer.len() < end {
                        self.data_buffer.resize(end, 0);
                    }
                    self.data_buffer[offset..end].copy_from_slice(&leaf.data);
                }
            }
        }

        self.size = Some(size);
        Ok(())
    }

    /// Check if this partial is complete.
    pub fn is_complete(&self) -> bool {
        self.bitfield.is_complete()
    }

    /// Update the bitfield and return whether it's now complete.
    pub fn update_bitfield(&mut self, new_bitfield: &Bitfield) -> bool {
        // Merge the new bitfield with our current one
        self.bitfield = new_bitfield.clone();
        self.bitfield.is_complete()
    }
}

/// The storage state for a blob.
#[derive(Default)]
pub enum BaoFileStorage {
    /// Initial state before loading.
    #[default]
    Initial,
    /// Currently loading from database/object storage.
    Loading,
    /// No entry exists for this hash.
    NonExisting,
    /// Blob is incomplete, data in memory buffer.
    PartialMem(PartialStorage),
    /// Blob is complete, data in object storage.
    Complete(CompleteStorage),
    /// An error occurred, storage is unusable.
    Poisoned,
}

impl fmt::Debug for BaoFileStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BaoFileStorage::Initial => write!(f, "Initial"),
            BaoFileStorage::Loading => write!(f, "Loading"),
            BaoFileStorage::NonExisting => write!(f, "NonExisting"),
            BaoFileStorage::PartialMem(p) => write!(f, "PartialMem({:?})", p.hash),
            BaoFileStorage::Complete(c) => write!(f, "Complete({}, {})", c.hash, c.size),
            BaoFileStorage::Poisoned => write!(f, "Poisoned"),
        }
    }
}

impl BaoFileStorage {
    /// Get the bitfield for this storage.
    pub fn bitfield(&self) -> Bitfield {
        match self {
            BaoFileStorage::Initial | BaoFileStorage::Loading => {
                panic!("storage not ready")
            }
            BaoFileStorage::NonExisting => Bitfield::empty(),
            BaoFileStorage::PartialMem(p) => p.bitfield.clone(),
            BaoFileStorage::Complete(c) => c.bitfield(),
            BaoFileStorage::Poisoned => {
                panic!("storage is poisoned")
            }
        }
    }

    /// Check if this is a complete blob.
    pub fn is_complete(&self) -> bool {
        matches!(self, BaoFileStorage::Complete(_))
    }

    /// Create a new partial mem storage.
    pub fn new_partial_mem(hash: String, size: Option<u64>) -> Self {
        Self::PartialMem(PartialStorage::new(hash, size))
    }
}

/// A reader that fetches data from object storage using range requests.
pub struct ObjectStoreDataReader {
    store: Arc<BlobObjectStore>,
    hash: String,
    size: u64,
}

impl ObjectStoreDataReader {
    pub fn new(store: Arc<BlobObjectStore>, hash: String, size: u64) -> Self {
        Self { store, hash, size }
    }

    /// Read bytes at the given offset.
    pub async fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        if offset >= self.size {
            return Ok(0);
        }

        let remaining = (self.size - offset) as usize;
        let len = buf.len().min(remaining);

        let bytes = self
            .store
            .get_data_range(&self.hash, offset as usize, len)
            .await
            .map_err(|e| io::Error::other(format!("object store read error: {}", e)))?;

        let read_len = bytes.len().min(buf.len());
        buf[..read_len].copy_from_slice(&bytes[..read_len]);

        Ok(read_len)
    }

    pub fn size(&self) -> u64 {
        self.size
    }
}

/// A reader that fetches outboard data from object storage.
pub struct ObjectStoreOutboardReader {
    store: Arc<BlobObjectStore>,
    hash: String,
}

impl ObjectStoreOutboardReader {
    pub fn new(store: Arc<BlobObjectStore>, hash: String) -> Self {
        Self { store, hash }
    }

    /// Read bytes at the given offset.
    pub async fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        let bytes = self
            .store
            .get_outboard_range(&self.hash, offset as usize, buf.len())
            .await
            .map_err(|e| io::Error::other(format!("object store outboard read error: {}", e)))?;

        let read_len = bytes.len().min(buf.len());
        buf[..read_len].copy_from_slice(&bytes[..read_len]);

        Ok(read_len)
    }
}

/// Calculate the size of the outboard for a given data size.
pub fn raw_outboard_size(size: u64) -> u64 {
    let tree = BaoTree::new(size, IROH_BLOCK_SIZE);
    tree.outboard_size()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complete_storage() {
        let storage = CompleteStorage::new("testhash".to_string(), 1024);
        assert_eq!(storage.size, 1024);
        assert!(!storage.has_outboard); // 1KB doesn't need outboard

        let storage_large = CompleteStorage::new("testhash".to_string(), 32 * 1024);
        assert!(storage_large.has_outboard); // 32KB needs outboard
    }

    #[test]
    fn test_partial_storage() {
        let storage = PartialStorage::new("testhash".to_string(), Some(1024));
        assert!(!storage.is_complete());
        assert_eq!(storage.size, Some(1024));
    }

    #[test]
    fn test_bao_file_storage_debug() {
        let initial = BaoFileStorage::Initial;
        assert!(format!("{:?}", initial).contains("Initial"));

        let complete = BaoFileStorage::Complete(CompleteStorage::new("abc".to_string(), 100));
        assert!(format!("{:?}", complete).contains("Complete"));
    }
}
