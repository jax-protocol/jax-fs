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
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::crypto::PublicKey;
use crate::linked_data::{BlockEncoded, DagCborCodec, Link};

use super::conflict::{
    operations_conflict, Conflict, ConflictResolver, MergeResult, Resolution, ResolvedConflict,
};

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

    /// Merge operations from another log with conflict resolution
    ///
    /// Unlike the basic `merge()`, this method:
    /// 1. Detects conflicts between local and incoming operations
    /// 2. Uses the provided resolver to decide how to handle each conflict
    /// 3. Returns detailed information about what conflicts were found and resolved
    ///
    /// # Arguments
    ///
    /// * `other` - The incoming log to merge
    /// * `resolver` - The conflict resolution strategy to use
    /// * `local_peer` - The local peer's identity (for tie-breaking)
    ///
    /// # Returns
    ///
    /// A `MergeResult` containing:
    /// - Number of operations added
    /// - List of resolved conflicts
    /// - List of unresolved conflicts (when using ForkOnConflict)
    pub fn merge_with_resolver(
        &mut self,
        other: &PathOpLog,
        resolver: &dyn ConflictResolver,
        local_peer: &PublicKey,
    ) -> MergeResult {
        let mut result = MergeResult::new();

        // First, identify all incoming operations that would conflict
        let mut conflicts_by_path: BTreeMap<PathBuf, Vec<(&OpId, &PathOperation)>> =
            BTreeMap::new();

        for (id, op) in &other.operations {
            // Skip if we already have this exact operation
            if self.operations.contains_key(id) {
                continue;
            }

            // Check for conflicts with existing operations on the same path
            let has_conflict = self
                .operations
                .values()
                .any(|existing| operations_conflict(existing, op));

            if has_conflict {
                conflicts_by_path
                    .entry(op.path.clone())
                    .or_default()
                    .push((id, op));
            }
        }

        // Process each incoming operation
        for (id, op) in &other.operations {
            // Skip if we already have this exact operation
            if self.operations.contains_key(id) {
                continue;
            }

            // Update local clock to stay ahead of all seen operations
            if id.timestamp >= self.local_clock {
                self.local_clock = id.timestamp + 1;
            }

            // Find conflicting base operation (if any)
            let conflicting_base = self
                .operations
                .values()
                .find(|existing| operations_conflict(existing, op));

            match conflicting_base {
                Some(base) => {
                    // We have a conflict
                    let conflict = Conflict::new(op.path.clone(), base.clone(), op.clone());
                    let resolution = resolver.resolve(&conflict, local_peer);

                    match resolution {
                        Resolution::UseBase => {
                            // Don't add the incoming operation
                            result.conflicts_resolved.push(ResolvedConflict {
                                conflict,
                                resolution,
                            });
                        }
                        Resolution::UseIncoming => {
                            // Add the incoming operation (it will win in resolve_path)
                            self.operations.insert(id.clone(), op.clone());
                            result.operations_added += 1;
                            result.conflicts_resolved.push(ResolvedConflict {
                                conflict,
                                resolution,
                            });
                        }
                        Resolution::KeepBoth => {
                            // Add the incoming operation and track as unresolved
                            self.operations.insert(id.clone(), op.clone());
                            result.operations_added += 1;
                            result.unresolved_conflicts.push(conflict);
                        }
                        Resolution::SkipBoth => {
                            // Don't add incoming, and remove base
                            // Note: We don't actually remove from operations to preserve history
                            // Instead, this could be used by higher-level logic
                            result.conflicts_resolved.push(ResolvedConflict {
                                conflict,
                                resolution,
                            });
                        }
                    }
                }
                None => {
                    // No conflict, just add the operation
                    self.operations.insert(id.clone(), op.clone());
                    result.operations_added += 1;
                }
            }
        }

        result
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
    fn test_merge_with_resolver_no_conflicts() {
        use super::super::conflict::LastWriteWins;

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "file1.txt", None, false);

        let mut log2 = PathOpLog::new();
        log2.record(peer2, OpType::Add, "file2.txt", None, false);

        let resolver = LastWriteWins::new();
        let result = log1.merge_with_resolver(&log2, &resolver, &peer1);

        assert_eq!(result.operations_added, 1);
        assert_eq!(result.conflicts_resolved.len(), 0);
        assert!(!result.has_unresolved());
        assert_eq!(log1.len(), 2);
    }

    #[test]
    fn test_merge_with_resolver_last_write_wins() {
        use super::super::conflict::{LastWriteWins, Resolution};

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "file.txt", None, false);

        let mut log2 = PathOpLog::new();
        // Simulate peer2 being "ahead" by using a higher timestamp
        log2.record(peer2, OpType::Add, "dummy", None, false); // ts=1
        log2.record(peer2, OpType::Remove, "file.txt", None, false); // ts=2

        let resolver = LastWriteWins::new();
        let result = log1.merge_with_resolver(&log2, &resolver, &peer1);

        // Should have 2 ops added (dummy and remove)
        // The remove conflicts with add, incoming (ts=2) > base (ts=1)
        assert_eq!(result.operations_added, 2);
        assert_eq!(result.conflicts_resolved.len(), 1);

        let resolved = &result.conflicts_resolved[0];
        assert_eq!(resolved.resolution, Resolution::UseIncoming);
    }

    #[test]
    fn test_merge_with_resolver_base_wins() {
        use super::super::conflict::{BaseWins, Resolution};

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "file.txt", None, false);

        let mut log2 = PathOpLog::new();
        log2.record(peer2, OpType::Remove, "file.txt", None, false);

        let resolver = BaseWins::new();
        let result = log1.merge_with_resolver(&log2, &resolver, &peer1);

        // With BaseWins, the incoming Remove should not be added
        assert_eq!(result.operations_added, 0);
        assert_eq!(result.conflicts_resolved.len(), 1);

        let resolved = &result.conflicts_resolved[0];
        assert_eq!(resolved.resolution, Resolution::UseBase);

        // Original operation should still be the only one for this path
        let resolved_path = log1.resolve_path("file.txt");
        assert!(matches!(resolved_path.unwrap().op_type, OpType::Add));
    }

    #[test]
    fn test_merge_with_resolver_fork_on_conflict() {
        use super::super::conflict::ForkOnConflict;

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "file.txt", None, false);

        let mut log2 = PathOpLog::new();
        log2.record(peer2, OpType::Add, "file.txt", None, false);

        let resolver = ForkOnConflict::new();
        let result = log1.merge_with_resolver(&log2, &resolver, &peer1);

        // With ForkOnConflict, the incoming operation should be added
        // but tracked as unresolved
        assert_eq!(result.operations_added, 1);
        assert_eq!(result.conflicts_resolved.len(), 0);
        assert!(result.has_unresolved());
        assert_eq!(result.unresolved_conflicts.len(), 1);

        // Both operations should be in the log
        assert_eq!(log1.len(), 2);

        // resolve_path should return the winner by OpId
        let resolved = log1.resolve_path("file.txt").unwrap();
        // The winner is the one with higher OpId
        if peer2 > peer1 {
            assert_eq!(resolved.id.peer_id, peer2);
        } else {
            assert_eq!(resolved.id.peer_id, peer1);
        }
    }

    #[test]
    fn test_merge_with_resolver_concurrent_ops() {
        use super::super::conflict::LastWriteWins;

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        // Both peers start with the same clock (concurrent operations)
        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "file.txt", None, false);

        let mut log2 = PathOpLog::new();
        log2.record(peer2, OpType::Add, "file.txt", None, false);

        // Both have timestamp=1, so this is a true concurrent edit
        let resolver = LastWriteWins::new();
        let result = log1.merge_with_resolver(&log2, &resolver, &peer1);

        // There's a conflict
        assert_eq!(result.total_conflicts(), 1);

        // The result depends on peer_id ordering
        // LastWriteWins uses OpId comparison (timestamp then peer_id)
        if peer2 > peer1 {
            // peer2 wins, incoming operation added
            assert_eq!(result.operations_added, 1);
        } else {
            // peer1 wins, incoming operation not added
            assert_eq!(result.operations_added, 0);
        }
    }

    #[test]
    fn test_merge_with_resolver_idempotent() {
        use super::super::conflict::LastWriteWins;

        let peer1 = make_peer_id(1);
        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "file.txt", None, false);

        let log1_clone = log1.clone();
        let resolver = LastWriteWins::new();
        let result = log1.merge_with_resolver(&log1_clone, &resolver, &peer1);

        // Merging with self should add nothing and have no conflicts
        assert_eq!(result.operations_added, 0);
        assert_eq!(result.total_conflicts(), 0);
        assert_eq!(log1.len(), 1);
    }

    #[test]
    fn test_merge_with_resolver_mixed_conflicts() {
        use super::super::conflict::LastWriteWins;

        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let mut log1 = PathOpLog::new();
        log1.record(peer1, OpType::Add, "file1.txt", None, false);
        log1.record(peer1, OpType::Add, "file2.txt", None, false);

        let mut log2 = PathOpLog::new();
        log2.record(peer2, OpType::Remove, "file1.txt", None, false); // conflicts
        log2.record(peer2, OpType::Add, "file3.txt", None, false); // no conflict

        let resolver = LastWriteWins::new();
        let result = log1.merge_with_resolver(&log2, &resolver, &peer1);

        // file3.txt should be added (no conflict)
        // file1.txt has a conflict
        assert_eq!(result.total_conflicts(), 1);

        // file3.txt is always added
        assert!(log1.resolve_path("file3.txt").is_some());
    }
}
