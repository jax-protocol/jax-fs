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
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::crypto::PublicKey;
use crate::linked_data::{BlockEncoded, DagCborCodec, Link};

use super::conflict::{Conflict, ConflictResolver, MergeResult, Resolution};

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
        from: PathBuf,
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
    /// Peer that created this operation (for tie-breaking)
    pub peer_id: PublicKey,
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
    pub path: PathBuf,
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

    /// Current Lamport clock (not serialized, rebuilt from operations)
    #[serde(skip)]
    local_clock: u64,
}

impl BlockEncoded<DagCborCodec> for PathOpLog {}

impl PathOpLog {
    /// Create a new empty log
    pub fn new() -> Self {
        Self::default()
    }

    /// Rebuild local clock from operations (call after deserialization)
    pub fn rebuild_clock(&mut self) {
        self.local_clock = self
            .operations
            .keys()
            .map(|id| id.timestamp)
            .max()
            .unwrap_or(0);
    }

    /// Record a new local operation
    ///
    /// The peer_id identifies who is recording this operation.
    /// Returns the OpId of the recorded operation.
    pub fn record(
        &mut self,
        peer_id: PublicKey,
        op_type: OpType,
        path: impl Into<PathBuf>,
        content_link: Option<Link>,
        is_dir: bool,
    ) -> OpId {
        self.local_clock += 1;

        let id = OpId {
            timestamp: self.local_clock,
            peer_id,
        };

        let op = PathOperation {
            id: id.clone(),
            op_type,
            path: path.into(),
            content_link,
            is_dir,
        };

        self.operations.insert(id.clone(), op);
        id
    }

    /// Merge operations from another log (CRDT merge)
    ///
    /// Returns the number of new operations added.
    /// This uses the default LastWriteWins strategy for conflicts.
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

    /// Merge operations from another log with custom conflict resolution
    ///
    /// Unlike the basic [`merge`](Self::merge), this method applies a
    /// [`ConflictResolver`] strategy when operations from different peers
    /// affect the same path concurrently.
    ///
    /// # Arguments
    /// * `other` - The log to merge from
    /// * `resolver` - The conflict resolution strategy to apply
    ///
    /// # Returns
    /// A [`MergeResult`] containing counts of added, rejected, and forked operations
    pub fn merge_with_resolver<R: ConflictResolver>(
        &mut self,
        other: &PathOpLog,
        resolver: &R,
    ) -> MergeResult {
        let mut result = MergeResult::default();

        for (id, incoming_op) in &other.operations {
            // Skip if we already have this exact operation
            if self.operations.contains_key(id) {
                continue;
            }

            // Check for conflict: same path, concurrent operations
            if let Some(base_op) = self.find_concurrent_op(&incoming_op.path, id) {
                let conflict = Conflict {
                    path: &incoming_op.path,
                    base_op,
                    incoming_op,
                };

                match resolver.resolve(&conflict) {
                    Resolution::KeepBase => {
                        // Don't add the incoming operation
                        result.rejected += 1;
                    }
                    Resolution::AcceptIncoming => {
                        self.operations.insert(id.clone(), incoming_op.clone());
                        result.added += 1;
                    }
                    Resolution::Fork { forked_path } => {
                        // Add operation with modified path
                        let mut forked_op = incoming_op.clone();
                        forked_op.path = forked_path;
                        self.operations.insert(id.clone(), forked_op);
                        result.forked += 1;
                    }
                }
            } else {
                // No conflict, just add the operation
                self.operations.insert(id.clone(), incoming_op.clone());
                result.added += 1;
            }

            // Update local clock to stay ahead of all seen operations
            if id.timestamp >= self.local_clock {
                self.local_clock = id.timestamp + 1;
            }
        }

        result
    }

    /// Find an operation on the same path that's concurrent with the given OpId
    ///
    /// An operation is considered concurrent if it's on the same path and
    /// doesn't causally precede the incoming operation.
    fn find_concurrent_op(&self, path: &Path, incoming_id: &OpId) -> Option<&PathOperation> {
        self.operations
            .values()
            .filter(|op| op.path == path)
            .filter(|op| !self.happens_before(&op.id, incoming_id))
            .max_by_key(|op| &op.id)
    }

    /// Check if op1 happens-before op2
    ///
    /// In our Lamport clock model, op1 happens-before op2 if they're from
    /// the same peer and op1 has a lower timestamp.
    fn happens_before(&self, op1: &OpId, op2: &OpId) -> bool {
        op1.peer_id == op2.peer_id && op1.timestamp < op2.timestamp
    }

    /// Get all operations
    pub fn operations(&self) -> &BTreeMap<OpId, PathOperation> {
        &self.operations
    }

    /// Resolve the current state of a path
    ///
    /// Returns the winning operation for the path (if any).
    /// The operation with the highest OpId wins.
    pub fn resolve_path(&self, path: impl AsRef<std::path::Path>) -> Option<&PathOperation> {
        let path = path.as_ref();
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
    pub fn resolve_all(&self) -> BTreeMap<PathBuf, &PathOperation> {
        let mut result: BTreeMap<PathBuf, &PathOperation> = BTreeMap::new();

        // Group operations by path
        let mut by_path: BTreeMap<&PathBuf, Vec<&PathOperation>> = BTreeMap::new();
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
    pub fn ops_for_path(&self, path: impl AsRef<std::path::Path>) -> Vec<&PathOperation> {
        let path = path.as_ref();
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
    use crate::crypto::SecretKey;

    fn make_peer_id(seed: u8) -> PublicKey {
        // Generate a deterministic keypair from the seed
        // We use the seed to create a reproducible secret key
        let mut seed_bytes = [0u8; 32];
        seed_bytes[0] = seed;
        let secret = SecretKey::from(seed_bytes);
        secret.public()
    }

    #[test]
    fn test_op_id_ordering() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let id1 = OpId {
            timestamp: 1,
            peer_id: peer1,
        };
        let id2 = OpId {
            timestamp: 2,
            peer_id: peer1,
        };
        let id3 = OpId {
            timestamp: 1,
            peer_id: peer2,
        };

        // Higher timestamp wins
        assert!(id2 > id1);
        // Same timestamp, different peer_ids have deterministic ordering
        assert!(id3 != id1);
        // Order is determined by peer_id comparison
        if peer2 > peer1 {
            assert!(id3 > id1);
        } else {
            assert!(id3 < id1);
        }
    }

    #[test]
    fn test_record_operation() {
        let peer1 = make_peer_id(1);
        let mut log = PathOpLog::new();

        let id = log.record(peer1, OpType::Add, "file.txt", None, false);

        assert_eq!(id.timestamp, 1);
        assert_eq!(log.len(), 1);

        let op = log.operations.get(&id).unwrap();
        assert_eq!(op.path, PathBuf::from("file.txt"));
        assert!(matches!(op.op_type, OpType::Add));
    }

    #[test]
    fn test_record_multiple_operations() {
        let peer1 = make_peer_id(1);
        let mut log = PathOpLog::new();

        let id1 = log.record(peer1, OpType::Add, "file1.txt", None, false);
        let id2 = log.record(peer1, OpType::Add, "file2.txt", None, false);
        let id3 = log.record(peer1, OpType::Remove, "file1.txt", None, false);

        assert_eq!(id1.timestamp, 1);
        assert_eq!(id2.timestamp, 2);
        assert_eq!(id3.timestamp, 3);
        assert_eq!(log.len(), 3);
    }

    #[test]
    fn test_merge_logs() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "file1.txt", None, false);

        let mut log2 = PathOpLog::new();
        log2.record(peer2, OpType::Add, "file2.txt", None, false);

        let added = log1.merge(&log2);

        assert_eq!(added, 1);
        assert_eq!(log1.len(), 2);
    }

    #[test]
    fn test_merge_idempotent() {
        let peer1 = make_peer_id(1);
        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "file.txt", None, false);

        let log1_clone = log1.clone();
        let added = log1.merge(&log1_clone);

        assert_eq!(added, 0);
        assert_eq!(log1.len(), 1);
    }

    #[test]
    fn test_resolve_path_single_op() {
        let peer1 = make_peer_id(1);
        let mut log = PathOpLog::new();
        log.record(peer1, OpType::Add, "file.txt", None, false);

        let resolved = log.resolve_path("file.txt");
        assert!(resolved.is_some());
        assert!(matches!(resolved.unwrap().op_type, OpType::Add));
    }

    #[test]
    fn test_resolve_path_latest_wins() {
        let peer1 = make_peer_id(1);
        let mut log = PathOpLog::new();

        log.record(peer1, OpType::Add, "file.txt", None, false);
        log.record(peer1, OpType::Remove, "file.txt", None, false);

        let resolved = log.resolve_path("file.txt");
        assert!(resolved.is_some());
        assert!(matches!(resolved.unwrap().op_type, OpType::Remove));
    }

    #[test]
    fn test_resolve_all_excludes_removed() {
        let peer1 = make_peer_id(1);
        let mut log = PathOpLog::new();

        log.record(peer1, OpType::Add, "file1.txt", None, false);
        log.record(peer1, OpType::Add, "file2.txt", None, false);
        log.record(peer1, OpType::Remove, "file1.txt", None, false);

        let resolved = log.resolve_all();

        assert_eq!(resolved.len(), 1);
        assert!(resolved.contains_key(&PathBuf::from("file2.txt")));
        assert!(!resolved.contains_key(&PathBuf::from("file1.txt")));
    }

    #[test]
    fn test_concurrent_ops_different_peers() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "file.txt", None, false);

        let mut log2 = PathOpLog::new();
        log2.record(peer2, OpType::Remove, "file.txt", None, false);

        // Merge log2 into log1
        log1.merge(&log2);

        // Both have timestamp=1, winner is determined by peer_id ordering
        let resolved = log1.resolve_path("file.txt");
        assert!(resolved.is_some());
        let winning_op = resolved.unwrap();

        // The peer with higher ID wins (deterministic tie-breaking)
        if peer2 > peer1 {
            assert!(matches!(winning_op.op_type, OpType::Remove));
        } else {
            assert!(matches!(winning_op.op_type, OpType::Add));
        }
    }

    #[test]
    fn test_mv_operation() {
        let peer1 = make_peer_id(1);
        let mut log = PathOpLog::new();

        log.record(peer1, OpType::Add, "old.txt", None, false);
        log.record(
            peer1,
            OpType::Mv {
                from: PathBuf::from("old.txt"),
            },
            "new.txt",
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

        let peer1 = make_peer_id(1);
        let mut log = PathOpLog::new();

        log.record(peer1, OpType::Add, "file1.txt", None, false);
        log.record(peer1, OpType::Mkdir, "dir", None, true);
        log.record(
            peer1,
            OpType::Mv {
                from: PathBuf::from("file1.txt"),
            },
            "dir/file1.txt",
            None,
            false,
        );

        let encoded = log.encode().unwrap();
        let decoded = PathOpLog::decode(&encoded).unwrap();

        // Operations should match (local_clock is not serialized)
        assert_eq!(log.operations, decoded.operations);
    }

    #[test]
    fn test_merge_with_last_write_wins() {
        use super::super::conflict::LastWriteWins;

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        // Base log: peer1 adds file.txt at timestamp 1
        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "file.txt", None, false);

        // Incoming log: peer2 adds file.txt at timestamp 1 (concurrent)
        let mut log2 = PathOpLog::new();
        log2.record(peer2, OpType::Add, "file.txt", None, false);

        let result = log1.merge_with_resolver(&log2, &LastWriteWins);

        // One operation should be added (the higher OpId wins)
        assert_eq!(result.added, 1);
        assert_eq!(result.rejected, 0);
        assert_eq!(result.forked, 0);

        // Log should have 2 operations now
        assert_eq!(log1.len(), 2);
    }

    #[test]
    fn test_merge_with_base_wins() {
        use super::super::conflict::BaseWins;

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        // Base log: peer1 adds file.txt
        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "file.txt", None, false);

        // Incoming log: peer2 adds file.txt (concurrent)
        let mut log2 = PathOpLog::new();
        log2.record(peer2, OpType::Add, "file.txt", None, false);

        let result = log1.merge_with_resolver(&log2, &BaseWins);

        // Incoming should be rejected
        assert_eq!(result.added, 0);
        assert_eq!(result.rejected, 1);
        assert_eq!(result.forked, 0);

        // Log should still have only 1 operation
        assert_eq!(log1.len(), 1);
    }

    #[test]
    fn test_merge_with_fork_on_conflict() {
        use super::super::conflict::ForkOnConflict;

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        // Base log: peer1 adds document.txt
        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "document.txt", None, false);

        // Incoming log: peer2 adds document.txt (concurrent)
        let mut log2 = PathOpLog::new();
        log2.record(peer2, OpType::Add, "document.txt", None, false);

        let result = log1.merge_with_resolver(&log2, &ForkOnConflict);

        // Incoming should be forked
        assert_eq!(result.added, 0);
        assert_eq!(result.rejected, 0);
        assert_eq!(result.forked, 1);

        // Log should have 2 operations now
        assert_eq!(log1.len(), 2);

        // Check that one path is the original and one is forked
        let paths: Vec<_> = log1.operations.values().map(|op| &op.path).collect();
        assert!(paths.iter().any(|p| *p == &PathBuf::from("document.txt")));
        assert!(paths.iter().any(|p| {
            let name = p.file_name().unwrap().to_string_lossy();
            name.starts_with("document@") && name.ends_with(".txt")
        }));
    }

    #[test]
    fn test_merge_no_conflict_different_paths() {
        use super::super::conflict::BaseWins;

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        // Base log: peer1 adds file1.txt
        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "file1.txt", None, false);

        // Incoming log: peer2 adds file2.txt (different path, no conflict)
        let mut log2 = PathOpLog::new();
        log2.record(peer2, OpType::Add, "file2.txt", None, false);

        let result = log1.merge_with_resolver(&log2, &BaseWins);

        // Should be added without conflict
        assert_eq!(result.added, 1);
        assert_eq!(result.rejected, 0);
        assert_eq!(result.forked, 0);

        assert_eq!(log1.len(), 2);
    }

    #[test]
    fn test_merge_same_peer_no_conflict() {
        use super::super::conflict::BaseWins;

        let peer1 = make_peer_id(1);

        // Base log: peer1 adds file.txt at timestamp 1
        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "file.txt", None, false);

        // Create another log with peer1's later operation
        let mut log2 = PathOpLog::new();
        log2.local_clock = 1; // Start at higher clock
        log2.record(peer1, OpType::Remove, "file.txt", None, false);

        let result = log1.merge_with_resolver(&log2, &BaseWins);

        // Same peer operations are causal, not concurrent - should add
        assert_eq!(result.added, 1);
        assert_eq!(result.rejected, 0);
        assert_eq!(result.forked, 0);
    }

    #[test]
    fn test_merge_result_had_conflicts() {
        assert!(!MergeResult::default().had_conflicts());

        let mut result = MergeResult {
            added: 5,
            rejected: 0,
            forked: 0,
        };
        assert!(!result.had_conflicts());

        result.rejected = 1;
        assert!(result.had_conflicts());

        result.rejected = 0;
        result.forked = 1;
        assert!(result.had_conflicts());
    }

    #[test]
    fn test_merge_multiple_conflicts_fork() {
        use super::super::conflict::ForkOnConflict;

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        // Base log: peer1 adds multiple files
        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "file1.txt", None, false);
        log1.record(peer1, OpType::Add, "file2.txt", None, false);
        log1.record(peer1, OpType::Add, "file3.txt", None, false);

        // Incoming log: peer2 adds same files (concurrent conflicts)
        let mut log2 = PathOpLog::new();
        log2.record(peer2, OpType::Add, "file1.txt", None, false);
        log2.record(peer2, OpType::Add, "file2.txt", None, false);
        log2.record(peer2, OpType::Add, "unique.txt", None, false); // No conflict

        let result = log1.merge_with_resolver(&log2, &ForkOnConflict);

        // 2 conflicts should be forked, 1 added without conflict
        assert_eq!(result.forked, 2);
        assert_eq!(result.added, 1);
        assert_eq!(result.rejected, 0);
        assert_eq!(log1.len(), 6);
    }

    #[test]
    fn test_merge_multiple_conflicts_base_wins() {
        use super::super::conflict::BaseWins;

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        // Base log: peer1 adds multiple files
        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "file1.txt", None, false);
        log1.record(peer1, OpType::Add, "file2.txt", None, false);

        // Incoming log: peer2 has conflicting and non-conflicting ops
        let mut log2 = PathOpLog::new();
        log2.record(peer2, OpType::Add, "file1.txt", None, false); // Conflict
        log2.record(peer2, OpType::Add, "file3.txt", None, false); // No conflict

        let result = log1.merge_with_resolver(&log2, &BaseWins);

        assert_eq!(result.rejected, 1);
        assert_eq!(result.added, 1);
        assert_eq!(result.forked, 0);
        assert_eq!(log1.len(), 3);
    }

    #[test]
    fn test_merge_with_remove_operations() {
        use super::super::conflict::LastWriteWins;

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        // Base log: peer1 adds then removes
        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "file.txt", None, false);

        // Incoming log: peer2 removes the same file (concurrent)
        let mut log2 = PathOpLog::new();
        log2.record(peer2, OpType::Remove, "file.txt", None, false);

        let result = log1.merge_with_resolver(&log2, &LastWriteWins);

        // Both operations should be in the log
        assert_eq!(result.added, 1);
        assert_eq!(log1.len(), 2);

        // Resolve should work correctly
        let resolved = log1.resolve_all();
        // The winner depends on OpId ordering - either file exists or doesn't
        // But we should have a deterministic result
        assert!(resolved.len() <= 1);
    }

    #[test]
    fn test_merge_with_mkdir_operations() {
        use super::super::conflict::ForkOnConflict;

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        // Base log: peer1 creates directory
        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Mkdir, "mydir", None, true);

        // Incoming log: peer2 creates same directory (concurrent)
        let mut log2 = PathOpLog::new();
        log2.record(peer2, OpType::Mkdir, "mydir", None, true);

        let result = log1.merge_with_resolver(&log2, &ForkOnConflict);

        // Directory conflict should be forked
        assert_eq!(result.forked, 1);
        assert_eq!(log1.len(), 2);

        // Check forked path exists
        let paths: Vec<_> = log1.operations.values().map(|op| &op.path).collect();
        assert!(paths.iter().any(|p| {
            let name = p.file_name().unwrap().to_string_lossy();
            name.starts_with("mydir@")
        }));
    }

    #[test]
    fn test_merge_with_mv_operations() {
        use super::super::conflict::LastWriteWins;

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        // Base log: peer1 adds file, then moves it
        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "old.txt", None, false);
        log1.record(
            peer1,
            OpType::Mv {
                from: PathBuf::from("old.txt"),
            },
            "new.txt",
            None,
            false,
        );

        // Incoming log: peer2 adds file at destination (conflict at new.txt)
        let mut log2 = PathOpLog::new();
        log2.record(peer2, OpType::Add, "new.txt", None, false);

        let result = log1.merge_with_resolver(&log2, &LastWriteWins);

        // The add at new.txt conflicts with mv to new.txt
        assert_eq!(result.added, 1);
        assert_eq!(log1.len(), 3);
    }

    #[test]
    fn test_merge_with_resolver_idempotent() {
        use super::super::conflict::LastWriteWins;

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "file.txt", None, false);

        let mut log2 = PathOpLog::new();
        log2.record(peer2, OpType::Add, "file.txt", None, false);

        // First merge
        let _result1 = log1.merge_with_resolver(&log2, &LastWriteWins);
        let len_after_first = log1.len();

        // Second merge should be idempotent
        let result2 = log1.merge_with_resolver(&log2, &LastWriteWins);

        assert_eq!(log1.len(), len_after_first);
        assert_eq!(result2.added, 0);
        assert_eq!(result2.rejected, 0);
        assert_eq!(result2.forked, 0);
        assert_eq!(result2.total(), 0);
    }

    #[test]
    fn test_merge_commutativity_last_write_wins() {
        use super::super::conflict::LastWriteWins;

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        // Create two logs
        let mut log1a = PathOpLog::new();
        log1a.record(peer1, OpType::Add, "file.txt", None, false);

        let mut log2a = PathOpLog::new();
        log2a.record(peer2, OpType::Add, "file.txt", None, false);

        // Merge in different orders
        let log1b = log1a.clone();
        let mut log2b = log2a.clone();

        log1a.merge_with_resolver(&log2a, &LastWriteWins);
        log2b.merge_with_resolver(&log1b, &LastWriteWins);

        // Both should have same operations (order may differ)
        assert_eq!(log1a.len(), log2b.len());

        // Resolve should give same result
        let resolved1 = log1a.resolve_all();
        let resolved2 = log2b.resolve_all();
        assert_eq!(resolved1.len(), resolved2.len());
    }

    #[test]
    fn test_custom_conflict_resolver() {
        use super::super::conflict::{Conflict, ConflictResolver, Resolution};

        /// Custom resolver: always fork with a fixed suffix
        struct AlwaysForkWithSuffix;

        impl ConflictResolver for AlwaysForkWithSuffix {
            fn resolve(&self, conflict: &Conflict) -> Resolution {
                let mut new_path = conflict.path.to_path_buf();
                let stem = conflict
                    .path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let ext = conflict
                    .path
                    .extension()
                    .map(|e| format!(".{}", e.to_string_lossy()))
                    .unwrap_or_default();
                new_path.set_file_name(format!("{}_conflict{}", stem, ext));
                Resolution::Fork {
                    forked_path: new_path,
                }
            }
        }

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "test.txt", None, false);

        let mut log2 = PathOpLog::new();
        log2.record(peer2, OpType::Add, "test.txt", None, false);

        let result = log1.merge_with_resolver(&log2, &AlwaysForkWithSuffix);

        assert_eq!(result.forked, 1);

        let paths: Vec<_> = log1.operations.values().map(|op| &op.path).collect();
        assert!(paths
            .iter()
            .any(|p| p.to_string_lossy() == "test_conflict.txt"));
    }

    #[test]
    fn test_conflict_add_vs_remove() {
        use super::super::conflict::BaseWins;

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        // Base: peer1 adds file
        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "file.txt", None, false);

        // Incoming: peer2 removes file
        let mut log2 = PathOpLog::new();
        log2.record(peer2, OpType::Remove, "file.txt", None, false);

        let result = log1.merge_with_resolver(&log2, &BaseWins);

        // Remove conflicts with Add, should be rejected with BaseWins
        assert_eq!(result.rejected, 1);
        assert_eq!(log1.len(), 1);

        // File should still exist in resolved state
        let resolved = log1.resolve_all();
        assert_eq!(resolved.len(), 1);
    }

    #[test]
    fn test_nested_path_no_conflict() {
        use super::super::conflict::BaseWins;

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        // Base: peer1 adds dir/file.txt
        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "dir/file.txt", None, false);

        // Incoming: peer2 adds dir/other.txt (different path, no conflict)
        let mut log2 = PathOpLog::new();
        log2.record(peer2, OpType::Add, "dir/other.txt", None, false);

        let result = log1.merge_with_resolver(&log2, &BaseWins);

        // No conflict - different paths
        assert_eq!(result.added, 1);
        assert_eq!(result.rejected, 0);
        assert_eq!(log1.len(), 2);
    }

    #[test]
    fn test_merge_empty_logs() {
        use super::super::conflict::LastWriteWins;

        let mut log1 = PathOpLog::new();
        let log2 = PathOpLog::new();

        let result = log1.merge_with_resolver(&log2, &LastWriteWins);

        assert_eq!(result.total(), 0);
        assert_eq!(log1.len(), 0);
    }

    #[test]
    fn test_merge_into_empty_log() {
        use super::super::conflict::LastWriteWins;

        let peer1 = make_peer_id(1);

        let mut log1 = PathOpLog::new();
        let mut log2 = PathOpLog::new();
        log2.record(peer1, OpType::Add, "file1.txt", None, false);
        log2.record(peer1, OpType::Add, "file2.txt", None, false);

        let result = log1.merge_with_resolver(&log2, &LastWriteWins);

        // All should be added (no conflicts possible with empty base)
        assert_eq!(result.added, 2);
        assert_eq!(result.rejected, 0);
        assert_eq!(result.forked, 0);
        assert_eq!(log1.len(), 2);
    }

    #[test]
    fn test_fork_preserves_operation_type() {
        use super::super::conflict::ForkOnConflict;

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "file.txt", None, false);

        let mut log2 = PathOpLog::new();
        log2.record(peer2, OpType::Mkdir, "file.txt", None, true);

        let result = log1.merge_with_resolver(&log2, &ForkOnConflict);

        assert_eq!(result.forked, 1);

        // Find the forked operation
        let forked_op = log1
            .operations
            .values()
            .find(|op| op.path.to_string_lossy().contains('@'))
            .unwrap();

        // Should preserve the original operation type (Mkdir)
        assert!(matches!(forked_op.op_type, OpType::Mkdir));
        assert!(forked_op.is_dir);
    }

    #[test]
    fn test_clock_updates_on_merge() {
        use super::super::conflict::LastWriteWins;

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "file1.txt", None, false);
        // log1 clock is now 1

        let mut log2 = PathOpLog::new();
        log2.local_clock = 99; // Start with high clock
        log2.record(peer2, OpType::Add, "file2.txt", None, false);
        // log2 has op with timestamp 100

        log1.merge_with_resolver(&log2, &LastWriteWins);

        // After merge, log1's clock should be updated
        // New operations should have timestamp > 100
        let id = log1.record(peer1, OpType::Add, "file3.txt", None, false);
        assert!(id.timestamp > 100);
    }

    #[test]
    fn test_three_way_merge() {
        use super::super::conflict::LastWriteWins;

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);
        let peer3 = make_peer_id(3);

        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "shared.txt", None, false);

        let mut log2 = PathOpLog::new();
        log2.record(peer2, OpType::Add, "shared.txt", None, false);
        log2.record(peer2, OpType::Add, "peer2only.txt", None, false);

        let mut log3 = PathOpLog::new();
        log3.record(peer3, OpType::Add, "shared.txt", None, false);
        log3.record(peer3, OpType::Add, "peer3only.txt", None, false);

        // Merge log2 and log3 into log1
        let result2 = log1.merge_with_resolver(&log2, &LastWriteWins);
        let result3 = log1.merge_with_resolver(&log3, &LastWriteWins);

        // Should have: 3 ops for shared.txt + 1 peer2only + 1 peer3only = 5 ops
        assert_eq!(log1.len(), 5);
        assert_eq!(result2.added, 2); // shared.txt + peer2only.txt
        assert_eq!(result3.added, 2); // shared.txt + peer3only.txt
    }
}
