//! Path Operation CRDT for tracking filesystem changes
//!
//! This module provides a Conflict-free Replicated Data Type (CRDT) for tracking
//! path operations (add, remove, mkdir, mv) across peers. The operation log enables:
//! - Filesystem history reconstruction
//! - Conflict resolution during peer sync
//!
//! The log is stored as an encrypted blob separate from the manifest to avoid
//! leaking directory structure information.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::crypto::PublicKey;
use crate::linked_data::{BlockEncoded, DagCborCodec, Link};

/// Type of path operation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpType {
    /// Add a new file
    Add,
    /// Remove a file or directory
    Remove,
    /// Create a directory
    Mkdir,
    /// Move/rename a file or directory
    Mv {
        /// Source path (the path being moved from)
        from: String,
    },
}

/// Operation identifier for causal ordering
///
/// Provides a total ordering across all operations from all peers:
/// - Primary ordering by Lamport timestamp
/// - Secondary ordering by peer_id (lexicographic) for tie-breaking
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OpId {
    /// Lamport timestamp (logical clock)
    pub timestamp: u64,
    /// Peer that created this operation (PublicKey bytes, for tie-breaking)
    pub peer_id: [u8; 32],
}

impl PartialOrd for OpId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OpId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.timestamp.cmp(&other.timestamp) {
            std::cmp::Ordering::Equal => self.peer_id.cmp(&other.peer_id),
            ord => ord,
        }
    }
}

/// A single path operation in the CRDT log
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathOperation {
    /// Unique operation ID (timestamp + peer_id)
    pub id: OpId,
    /// Type of operation
    pub op_type: OpType,
    /// Target path (destination for Mv, affected path for others)
    pub path: String,
    /// For Add: link to the content (None for Remove/Mkdir/Mv)
    pub content_link: Option<Link>,
    /// Whether this operation affects a directory
    pub is_dir: bool,
}

/// The path operation log - an operation-based CRDT
///
/// This is a grow-only set of operations with causal ordering.
/// Conflict resolution is deterministic based on:
/// 1. Lamport timestamp (higher wins for concurrent ops on same path)
/// 2. Peer ID (lexicographic tie-breaker)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PathOpLog {
    /// All operations, keyed by OpId for efficient lookup and ordering
    operations: BTreeMap<OpId, PathOperation>,

    /// Current Lamport clock for this peer (not serialized)
    #[serde(skip)]
    local_clock: u64,

    /// Local peer ID (not serialized, set when loading)
    #[serde(skip)]
    local_peer_id: Option<[u8; 32]>,
}

impl BlockEncoded<DagCborCodec> for PathOpLog {}

impl PathOpLog {
    /// Create a new empty log
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new log with the local peer ID set
    pub fn with_peer_id(peer_id: PublicKey) -> Self {
        let mut log = Self::new();
        log.local_peer_id = Some(peer_id.to_bytes());
        log
    }

    /// Set the local peer ID (needed after deserialization)
    pub fn set_peer_id(&mut self, peer_id: PublicKey) {
        self.local_peer_id = Some(peer_id.to_bytes());
        // Update local clock to be greater than any seen operation
        self.local_clock = self
            .operations
            .keys()
            .map(|id| id.timestamp)
            .max()
            .unwrap_or(0);
    }

    /// Record a new local operation
    ///
    /// Returns the OpId of the recorded operation
    pub fn record(
        &mut self,
        op_type: OpType,
        path: String,
        content_link: Option<Link>,
        is_dir: bool,
    ) -> OpId {
        self.local_clock += 1;
        let peer_id = self
            .local_peer_id
            .expect("peer_id must be set before recording operations");

        let id = OpId {
            timestamp: self.local_clock,
            peer_id,
        };

        let op = PathOperation {
            id: id.clone(),
            op_type,
            path,
            content_link,
            is_dir,
        };

        self.operations.insert(id.clone(), op);
        id
    }

    /// Merge operations from another log (CRDT merge)
    ///
    /// Returns the number of new operations added
    pub fn merge(&mut self, other: &PathOpLog) -> usize {
        let mut added = 0;
        for (id, op) in &other.operations {
            if !self.operations.contains_key(id) {
                self.operations.insert(id.clone(), op.clone());
                added += 1;
                // Update local clock to stay ahead of all seen operations
                if id.timestamp >= self.local_clock {
                    self.local_clock = id.timestamp + 1;
                }
            }
        }
        added
    }

    /// Get all operations
    pub fn operations(&self) -> &BTreeMap<OpId, PathOperation> {
        &self.operations
    }

    /// Resolve the current state of a path
    ///
    /// Returns the winning operation for the path (if any).
    /// The operation with the highest OpId wins.
    pub fn resolve_path(&self, path: &str) -> Option<&PathOperation> {
        // Find all operations affecting this path
        let path_ops: Vec<&PathOperation> = self
            .operations
            .values()
            .filter(|op| op.path == path)
            .collect();

        if path_ops.is_empty() {
            return None;
        }

        // The operation with the highest OpId wins (total order)
        path_ops.into_iter().max_by_key(|op| &op.id)
    }

    /// Resolve the entire filesystem state from operations
    ///
    /// Returns a map of path -> winning operation for all paths
    /// that currently exist (excludes paths where Remove was the winning op)
    pub fn resolve_all(&self) -> BTreeMap<String, &PathOperation> {
        let mut result: BTreeMap<String, &PathOperation> = BTreeMap::new();

        // Group operations by path
        let mut by_path: BTreeMap<&String, Vec<&PathOperation>> = BTreeMap::new();
        for op in self.operations.values() {
            by_path.entry(&op.path).or_default().push(op);
        }

        // For each path, pick the winning operation
        for (path, ops) in by_path {
            if let Some(winner) = ops.into_iter().max_by_key(|op| &op.id) {
                // Only include if the winning op is not a Remove
                if !matches!(winner.op_type, OpType::Remove) {
                    result.insert(path.clone(), winner);
                }
            }
        }

        result
    }

    /// Get the number of operations
    pub fn len(&self) -> usize {
        self.operations.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Get operations affecting a specific path, in order
    pub fn ops_for_path(&self, path: &str) -> Vec<&PathOperation> {
        self.operations
            .values()
            .filter(|op| op.path == path)
            .collect()
    }

    /// Get all operations in order (oldest to newest)
    pub fn ops_in_order(&self) -> impl Iterator<Item = &PathOperation> {
        self.operations.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_peer_id(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    #[test]
    fn test_op_id_ordering() {
        let id1 = OpId {
            timestamp: 1,
            peer_id: make_peer_id(1),
        };
        let id2 = OpId {
            timestamp: 2,
            peer_id: make_peer_id(1),
        };
        let id3 = OpId {
            timestamp: 1,
            peer_id: make_peer_id(2),
        };

        // Higher timestamp wins
        assert!(id2 > id1);
        // Same timestamp, higher peer_id wins
        assert!(id3 > id1);
    }

    #[test]
    fn test_record_operation() {
        let mut log = PathOpLog::new();
        log.local_peer_id = Some(make_peer_id(1));

        let id = log.record(OpType::Add, "file.txt".to_string(), None, false);

        assert_eq!(id.timestamp, 1);
        assert_eq!(log.len(), 1);

        let op = log.operations.get(&id).unwrap();
        assert_eq!(op.path, "file.txt");
        assert!(matches!(op.op_type, OpType::Add));
    }

    #[test]
    fn test_record_multiple_operations() {
        let mut log = PathOpLog::new();
        log.local_peer_id = Some(make_peer_id(1));

        let id1 = log.record(OpType::Add, "file1.txt".to_string(), None, false);
        let id2 = log.record(OpType::Add, "file2.txt".to_string(), None, false);
        let id3 = log.record(OpType::Remove, "file1.txt".to_string(), None, false);

        assert_eq!(id1.timestamp, 1);
        assert_eq!(id2.timestamp, 2);
        assert_eq!(id3.timestamp, 3);
        assert_eq!(log.len(), 3);
    }

    #[test]
    fn test_merge_logs() {
        let mut log1 = PathOpLog::new();
        log1.local_peer_id = Some(make_peer_id(1));
        log1.record(OpType::Add, "file1.txt".to_string(), None, false);

        let mut log2 = PathOpLog::new();
        log2.local_peer_id = Some(make_peer_id(2));
        log2.record(OpType::Add, "file2.txt".to_string(), None, false);

        let added = log1.merge(&log2);

        assert_eq!(added, 1);
        assert_eq!(log1.len(), 2);
    }

    #[test]
    fn test_merge_idempotent() {
        let mut log1 = PathOpLog::new();
        log1.local_peer_id = Some(make_peer_id(1));
        log1.record(OpType::Add, "file.txt".to_string(), None, false);

        let log1_clone = log1.clone();
        let added = log1.merge(&log1_clone);

        assert_eq!(added, 0);
        assert_eq!(log1.len(), 1);
    }

    #[test]
    fn test_resolve_path_single_op() {
        let mut log = PathOpLog::new();
        log.local_peer_id = Some(make_peer_id(1));
        log.record(OpType::Add, "file.txt".to_string(), None, false);

        let resolved = log.resolve_path("file.txt");
        assert!(resolved.is_some());
        assert!(matches!(resolved.unwrap().op_type, OpType::Add));
    }

    #[test]
    fn test_resolve_path_latest_wins() {
        let mut log = PathOpLog::new();
        log.local_peer_id = Some(make_peer_id(1));

        log.record(OpType::Add, "file.txt".to_string(), None, false);
        log.record(OpType::Remove, "file.txt".to_string(), None, false);

        let resolved = log.resolve_path("file.txt");
        assert!(resolved.is_some());
        assert!(matches!(resolved.unwrap().op_type, OpType::Remove));
    }

    #[test]
    fn test_resolve_all_excludes_removed() {
        let mut log = PathOpLog::new();
        log.local_peer_id = Some(make_peer_id(1));

        log.record(OpType::Add, "file1.txt".to_string(), None, false);
        log.record(OpType::Add, "file2.txt".to_string(), None, false);
        log.record(OpType::Remove, "file1.txt".to_string(), None, false);

        let resolved = log.resolve_all();

        assert_eq!(resolved.len(), 1);
        assert!(resolved.contains_key("file2.txt"));
        assert!(!resolved.contains_key("file1.txt"));
    }

    #[test]
    fn test_concurrent_ops_different_peers() {
        let mut log1 = PathOpLog::new();
        log1.local_peer_id = Some(make_peer_id(1));
        log1.record(OpType::Add, "file.txt".to_string(), None, false);

        let mut log2 = PathOpLog::new();
        log2.local_peer_id = Some(make_peer_id(2));
        log2.record(OpType::Remove, "file.txt".to_string(), None, false);

        // Merge log2 into log1
        log1.merge(&log2);

        // Both have timestamp=1, but peer_id 2 > peer_id 1, so Remove wins
        let resolved = log1.resolve_path("file.txt");
        assert!(resolved.is_some());
        assert!(matches!(resolved.unwrap().op_type, OpType::Remove));
    }

    #[test]
    fn test_mv_operation() {
        let mut log = PathOpLog::new();
        log.local_peer_id = Some(make_peer_id(1));

        log.record(OpType::Add, "old.txt".to_string(), None, false);
        log.record(
            OpType::Mv {
                from: "old.txt".to_string(),
            },
            "new.txt".to_string(),
            None,
            false,
        );

        assert_eq!(log.len(), 2);

        // The destination path should resolve to Mv
        let resolved = log.resolve_path("new.txt");
        assert!(resolved.is_some());
        assert!(matches!(resolved.unwrap().op_type, OpType::Mv { .. }));
    }

    #[test]
    fn test_serialization_roundtrip() {
        use crate::linked_data::BlockEncoded;

        let mut log = PathOpLog::new();
        log.local_peer_id = Some(make_peer_id(1));

        log.record(OpType::Add, "file1.txt".to_string(), None, false);
        log.record(OpType::Mkdir, "dir".to_string(), None, true);
        log.record(
            OpType::Mv {
                from: "file1.txt".to_string(),
            },
            "dir/file1.txt".to_string(),
            None,
            false,
        );

        let encoded = log.encode().unwrap();
        let decoded = PathOpLog::decode(&encoded).unwrap();

        // Operations should match (local_clock and local_peer_id are not serialized)
        assert_eq!(log.operations, decoded.operations);
    }
}
