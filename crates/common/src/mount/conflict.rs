//! Conflict resolution for PathOpLog merges
//!
//! This module provides pluggable conflict resolution strategies for handling
//! concurrent edits from different peers. When two peers edit the same path
//! concurrently, the resolver determines how to reconcile the conflict.
//!
//! # Built-in Strategies
//!
//! - **[`LastWriteWins`]**: Higher timestamp wins (default CRDT behavior)
//! - **[`BaseWins`]**: Local operations win over incoming ones
//! - **[`ForkOnConflict`]**: Keep both versions, returning conflicts for manual resolution
//! - **[`ConflictFile`]**: Rename incoming to `<name>@<timestamp>` to preserve both versions
//!
//! # Custom Resolvers
//!
//! Implement the [`ConflictResolver`] trait to create custom resolution strategies.

use std::path::PathBuf;

use crate::crypto::PublicKey;

use super::{OpType, PathOperation};

/// A detected conflict between two operations on the same path
#[derive(Debug, Clone)]
pub struct Conflict {
    /// The path where the conflict occurred
    pub path: PathBuf,
    /// The local (base) operation
    pub base: PathOperation,
    /// The incoming (remote) operation
    pub incoming: PathOperation,
}

impl Conflict {
    /// Create a new conflict
    pub fn new(path: PathBuf, base: PathOperation, incoming: PathOperation) -> Self {
        Self {
            path,
            base,
            incoming,
        }
    }

    /* Getters */

    /// Check if both operations have the same timestamp (true concurrent edit)
    pub fn is_concurrent(&self) -> bool {
        self.base.id.timestamp == self.incoming.id.timestamp
    }

    /// Get the operation with the higher OpId (the "winner" by default CRDT rules)
    pub fn crdt_winner(&self) -> &PathOperation {
        if self.incoming.id > self.base.id {
            &self.incoming
        } else {
            &self.base
        }
    }
}

/// Resolution decision for a conflict
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Use the base (local) operation
    UseBase,
    /// Use the incoming (remote) operation
    UseIncoming,
    /// Keep both operations (fork state)
    KeepBoth,
    /// Skip both operations (neither is applied)
    SkipBoth,
    /// Rename the incoming operation to a new path (creates a conflict file)
    RenameIncoming {
        /// The new path for the incoming operation
        new_path: PathBuf,
    },
}

/// Result of a merge operation with conflict information
#[derive(Debug, Clone)]
pub struct MergeResult {
    /// Number of operations added from the incoming log
    pub operations_added: usize,
    /// Conflicts that were resolved
    pub conflicts_resolved: Vec<ResolvedConflict>,
    /// Conflicts that could not be auto-resolved (when using ForkOnConflict)
    pub unresolved_conflicts: Vec<Conflict>,
}

impl MergeResult {
    /// Create a new merge result
    pub fn new() -> Self {
        Self {
            operations_added: 0,
            conflicts_resolved: Vec::new(),
            unresolved_conflicts: Vec::new(),
        }
    }

    /* Getters */

    /// Check if there were any unresolved conflicts
    pub fn has_unresolved(&self) -> bool {
        !self.unresolved_conflicts.is_empty()
    }

    /// Total number of conflicts (resolved + unresolved)
    pub fn total_conflicts(&self) -> usize {
        self.conflicts_resolved.len() + self.unresolved_conflicts.len()
    }
}

impl Default for MergeResult {
    fn default() -> Self {
        Self::new()
    }
}

/// A conflict that was resolved
#[derive(Debug, Clone)]
pub struct ResolvedConflict {
    /// The original conflict
    pub conflict: Conflict,
    /// How it was resolved
    pub resolution: Resolution,
}

/// Trait for conflict resolution strategies
///
/// Implementors define how to resolve conflicts when merging PathOpLogs
/// from different peers.
pub trait ConflictResolver: std::fmt::Debug + Send + Sync {
    /// Resolve a conflict between two operations on the same path
    ///
    /// # Arguments
    ///
    /// * `conflict` - The conflict to resolve
    /// * `local_peer` - The local peer's identity (useful for deterministic tie-breaking)
    ///
    /// # Returns
    ///
    /// The resolution decision
    fn resolve(&self, conflict: &Conflict, local_peer: &PublicKey) -> Resolution;
}

/// Last-write-wins conflict resolution (default CRDT behavior)
///
/// The operation with the highest OpId wins:
/// 1. Higher Lamport timestamp wins
/// 2. If timestamps are equal, higher peer_id wins (lexicographic)
///
/// This is deterministic and matches the default PathOpLog behavior.
#[derive(Debug, Clone, Default)]
pub struct LastWriteWins;

impl LastWriteWins {
    /// Create a new LastWriteWins resolver
    pub fn new() -> Self {
        Self
    }
}

impl ConflictResolver for LastWriteWins {
    fn resolve(&self, conflict: &Conflict, _local_peer: &PublicKey) -> Resolution {
        if conflict.incoming.id > conflict.base.id {
            Resolution::UseIncoming
        } else {
            Resolution::UseBase
        }
    }
}

/// Base-wins conflict resolution (conservative)
///
/// The local (base) operation always wins over incoming operations.
/// Use this when you want to preserve local changes and require
/// explicit action to accept remote changes.
#[derive(Debug, Clone, Default)]
pub struct BaseWins;

impl BaseWins {
    /// Create a new BaseWins resolver
    pub fn new() -> Self {
        Self
    }
}

impl ConflictResolver for BaseWins {
    fn resolve(&self, _conflict: &Conflict, _local_peer: &PublicKey) -> Resolution {
        Resolution::UseBase
    }
}

/// Fork-on-conflict resolution
///
/// Keeps both operations in the log when a conflict is detected.
/// Conflicts are tracked and returned for manual resolution later.
///
/// This is useful when automatic conflict resolution is not acceptable
/// and users need to manually choose which version to keep.
#[derive(Debug, Clone, Default)]
pub struct ForkOnConflict;

impl ForkOnConflict {
    /// Create a new ForkOnConflict resolver
    pub fn new() -> Self {
        Self
    }
}

impl ConflictResolver for ForkOnConflict {
    fn resolve(&self, _conflict: &Conflict, _local_peer: &PublicKey) -> Resolution {
        Resolution::KeepBoth
    }
}

/// Conflict-file resolution (recommended for peer sync)
///
/// When a conflict is detected, renames the incoming file to include a version
/// suffix: `<name>@<timestamp>.<ext>` or `<name>@<timestamp>` for files without
/// extensions.
///
/// This preserves both versions:
/// - The base (local) operation wins and keeps the original path
/// - The incoming operation is renamed to a conflict file
///
/// Users can then manually review and resolve the conflict files.
///
/// # Example
///
/// If `document.txt` conflicts:
/// - Local version stays at `document.txt`
/// - Incoming version becomes `document@1234567890.txt`
#[derive(Debug, Clone, Default)]
pub struct ConflictFile;

impl ConflictFile {
    /// Create a new ConflictFile resolver
    pub fn new() -> Self {
        Self
    }

    /// Generate a conflict filename for the incoming operation
    ///
    /// Format: `<stem>@<timestamp>.<ext>` or `<stem>@<timestamp>` if no extension
    pub fn conflict_path(path: &std::path::Path, timestamp: u64) -> PathBuf {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
        let ext = path.extension().and_then(|e| e.to_str());

        let conflict_name = match ext {
            Some(ext) => format!("{}@{}.{}", stem, timestamp, ext),
            None => format!("{}@{}", stem, timestamp),
        };

        match path.parent() {
            Some(parent) if parent != std::path::Path::new("") => parent.join(conflict_name),
            _ => PathBuf::from(conflict_name),
        }
    }
}

impl ConflictResolver for ConflictFile {
    fn resolve(&self, conflict: &Conflict, _local_peer: &PublicKey) -> Resolution {
        // Only create conflict files for Add operations (file content conflicts)
        // For Remove or Mv, use standard CRDT resolution
        match (&conflict.base.op_type, &conflict.incoming.op_type) {
            (OpType::Add, OpType::Add) => {
                // Both are adds - create a conflict file for incoming
                let new_path =
                    Self::conflict_path(&conflict.incoming.path, conflict.incoming.id.timestamp);
                Resolution::RenameIncoming { new_path }
            }
            _ => {
                // For other conflicts (Remove, Mv), fall back to last-write-wins
                if conflict.incoming.id > conflict.base.id {
                    Resolution::UseIncoming
                } else {
                    Resolution::UseBase
                }
            }
        }
    }
}

/// Check if two operations conflict
///
/// Two operations conflict if:
/// 1. They affect the same path
/// 2. They have different OpIds
/// 3. At least one is a destructive operation (Remove, Mv, or Add that overwrites)
pub fn operations_conflict(base: &PathOperation, incoming: &PathOperation) -> bool {
    // Same OpId means same operation, no conflict
    if base.id == incoming.id {
        return false;
    }

    // Must affect the same path
    if base.path != incoming.path {
        return false;
    }

    // Check if either operation is destructive
    let base_destructive = is_destructive(&base.op_type);
    let incoming_destructive = is_destructive(&incoming.op_type);

    // Conflict if either is destructive, or both are Add (concurrent creates)
    base_destructive
        || incoming_destructive
        || (matches!(base.op_type, OpType::Add) && matches!(incoming.op_type, OpType::Add))
}

/// Check if an operation type is destructive
fn is_destructive(op_type: &OpType) -> bool {
    matches!(op_type, OpType::Remove | OpType::Mv { .. })
}

/// Check if an operation at this path would conflict with a move operation
///
/// Move operations are special because they affect two paths: source and destination.
/// This checks if an operation conflicts with a move's source path.
pub fn conflicts_with_mv_source(op: &PathOperation, mv_from: &PathBuf) -> bool {
    // Check if op affects the mv source path
    &op.path == mv_from
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::SecretKey;
    use crate::mount::OpId;

    fn make_peer_id(seed: u8) -> PublicKey {
        let mut seed_bytes = [0u8; 32];
        seed_bytes[0] = seed;
        let secret = SecretKey::from(seed_bytes);
        secret.public()
    }

    fn make_op(peer_id: PublicKey, timestamp: u64, op_type: OpType, path: &str) -> PathOperation {
        PathOperation {
            id: OpId { timestamp, peer_id },
            op_type,
            path: PathBuf::from(path),
            content_link: None,
            is_dir: false,
        }
    }

    #[test]
    fn test_conflict_detection() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let op1 = make_op(peer1, 1, OpType::Add, "file.txt");
        let op2 = make_op(peer2, 1, OpType::Add, "file.txt");

        // Same path, different peers, both Add -> conflict
        assert!(operations_conflict(&op1, &op2));
    }

    #[test]
    fn test_no_conflict_different_paths() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let op1 = make_op(peer1, 1, OpType::Add, "file1.txt");
        let op2 = make_op(peer2, 1, OpType::Add, "file2.txt");

        // Different paths -> no conflict
        assert!(!operations_conflict(&op1, &op2));
    }

    #[test]
    fn test_no_conflict_same_operation() {
        let peer1 = make_peer_id(1);

        let op1 = make_op(peer1, 1, OpType::Add, "file.txt");
        let op2 = op1.clone();

        // Same OpId -> no conflict (same operation)
        assert!(!operations_conflict(&op1, &op2));
    }

    #[test]
    fn test_conflict_add_vs_remove() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let op1 = make_op(peer1, 1, OpType::Add, "file.txt");
        let op2 = make_op(peer2, 1, OpType::Remove, "file.txt");

        // Add vs Remove on same path -> conflict
        assert!(operations_conflict(&op1, &op2));
    }

    #[test]
    fn test_conflict_mkdir_vs_remove() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let op1 = make_op(peer1, 1, OpType::Mkdir, "dir");
        let op2 = make_op(peer2, 1, OpType::Remove, "dir");

        // Mkdir vs Remove on same path -> conflict
        assert!(operations_conflict(&op1, &op2));
    }

    #[test]
    fn test_no_conflict_mkdir_vs_mkdir() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let op1 = make_op(peer1, 1, OpType::Mkdir, "dir");
        let op2 = make_op(peer2, 1, OpType::Mkdir, "dir");

        // Mkdir vs Mkdir is idempotent -> no conflict
        assert!(!operations_conflict(&op1, &op2));
    }

    #[test]
    fn test_last_write_wins_incoming() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let base = make_op(peer1, 1, OpType::Add, "file.txt");
        let incoming = make_op(peer2, 2, OpType::Remove, "file.txt");

        let conflict = Conflict::new(PathBuf::from("file.txt"), base, incoming);
        let resolver = LastWriteWins::new();

        // Incoming has higher timestamp -> UseIncoming
        assert_eq!(resolver.resolve(&conflict, &peer1), Resolution::UseIncoming);
    }

    #[test]
    fn test_last_write_wins_base() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let base = make_op(peer1, 2, OpType::Add, "file.txt");
        let incoming = make_op(peer2, 1, OpType::Remove, "file.txt");

        let conflict = Conflict::new(PathBuf::from("file.txt"), base, incoming);
        let resolver = LastWriteWins::new();

        // Base has higher timestamp -> UseBase
        assert_eq!(resolver.resolve(&conflict, &peer1), Resolution::UseBase);
    }

    #[test]
    fn test_last_write_wins_tiebreak_by_peer_id() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let base = make_op(peer1, 1, OpType::Add, "file.txt");
        let incoming = make_op(peer2, 1, OpType::Remove, "file.txt");

        let conflict = Conflict::new(PathBuf::from("file.txt"), base, incoming);
        let resolver = LastWriteWins::new();

        let resolution = resolver.resolve(&conflict, &peer1);

        // Same timestamp, peer2 > peer1 (usually) -> check based on actual ordering
        if peer2 > peer1 {
            assert_eq!(resolution, Resolution::UseIncoming);
        } else {
            assert_eq!(resolution, Resolution::UseBase);
        }
    }

    #[test]
    fn test_base_wins() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let base = make_op(peer1, 1, OpType::Add, "file.txt");
        let incoming = make_op(peer2, 2, OpType::Remove, "file.txt");

        let conflict = Conflict::new(PathBuf::from("file.txt"), base, incoming);
        let resolver = BaseWins::new();

        // BaseWins always returns UseBase regardless of timestamps
        assert_eq!(resolver.resolve(&conflict, &peer1), Resolution::UseBase);
    }

    #[test]
    fn test_fork_on_conflict() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let base = make_op(peer1, 1, OpType::Add, "file.txt");
        let incoming = make_op(peer2, 2, OpType::Remove, "file.txt");

        let conflict = Conflict::new(PathBuf::from("file.txt"), base, incoming);
        let resolver = ForkOnConflict::new();

        // ForkOnConflict always returns KeepBoth
        assert_eq!(resolver.resolve(&conflict, &peer1), Resolution::KeepBoth);
    }

    #[test]
    fn test_conflict_is_concurrent() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let base = make_op(peer1, 5, OpType::Add, "file.txt");
        let incoming = make_op(peer2, 5, OpType::Remove, "file.txt");

        let conflict = Conflict::new(PathBuf::from("file.txt"), base, incoming);

        // Same timestamp -> concurrent
        assert!(conflict.is_concurrent());
    }

    #[test]
    fn test_conflict_not_concurrent() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let base = make_op(peer1, 3, OpType::Add, "file.txt");
        let incoming = make_op(peer2, 5, OpType::Remove, "file.txt");

        let conflict = Conflict::new(PathBuf::from("file.txt"), base, incoming);

        // Different timestamps -> not concurrent
        assert!(!conflict.is_concurrent());
    }

    #[test]
    fn test_crdt_winner() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let base = make_op(peer1, 3, OpType::Add, "file.txt");
        let incoming = make_op(peer2, 5, OpType::Remove, "file.txt");

        let conflict = Conflict::new(PathBuf::from("file.txt"), base, incoming.clone());

        // Incoming has higher timestamp
        assert_eq!(conflict.crdt_winner().id, incoming.id);
    }

    #[test]
    fn test_merge_result() {
        let mut result = MergeResult::new();

        assert_eq!(result.operations_added, 0);
        assert!(!result.has_unresolved());
        assert_eq!(result.total_conflicts(), 0);

        // Add an unresolved conflict
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);
        let base = make_op(peer1, 1, OpType::Add, "file.txt");
        let incoming = make_op(peer2, 1, OpType::Add, "file.txt");

        result
            .unresolved_conflicts
            .push(Conflict::new(PathBuf::from("file.txt"), base, incoming));

        assert!(result.has_unresolved());
        assert_eq!(result.total_conflicts(), 1);
    }

    #[test]
    fn test_conflict_file_path_with_extension() {
        let path = PathBuf::from("document.txt");
        let result = ConflictFile::conflict_path(&path, 1234567890);
        assert_eq!(result, PathBuf::from("document@1234567890.txt"));
    }

    #[test]
    fn test_conflict_file_path_without_extension() {
        let path = PathBuf::from("README");
        let result = ConflictFile::conflict_path(&path, 1234567890);
        assert_eq!(result, PathBuf::from("README@1234567890"));
    }

    #[test]
    fn test_conflict_file_path_nested() {
        let path = PathBuf::from("docs/notes/file.md");
        let result = ConflictFile::conflict_path(&path, 42);
        assert_eq!(result, PathBuf::from("docs/notes/file@42.md"));
    }

    #[test]
    fn test_conflict_file_resolver_add_vs_add() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let base = make_op(peer1, 1, OpType::Add, "file.txt");
        let incoming = make_op(peer2, 100, OpType::Add, "file.txt");

        let conflict = Conflict::new(PathBuf::from("file.txt"), base, incoming);
        let resolver = ConflictFile::new();

        let resolution = resolver.resolve(&conflict, &peer1);

        // Should rename incoming to conflict file
        match resolution {
            Resolution::RenameIncoming { new_path } => {
                assert_eq!(new_path, PathBuf::from("file@100.txt"));
            }
            _ => panic!("Expected RenameIncoming, got {:?}", resolution),
        }
    }

    #[test]
    fn test_conflict_file_resolver_add_vs_remove() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let base = make_op(peer1, 1, OpType::Add, "file.txt");
        let incoming = make_op(peer2, 100, OpType::Remove, "file.txt");

        let conflict = Conflict::new(PathBuf::from("file.txt"), base, incoming);
        let resolver = ConflictFile::new();

        // Non-Add conflicts fall back to last-write-wins
        // incoming (ts=100) > base (ts=1)
        assert_eq!(resolver.resolve(&conflict, &peer1), Resolution::UseIncoming);
    }

    #[test]
    fn test_conflict_file_resolver_remove_vs_add() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let base = make_op(peer1, 100, OpType::Remove, "file.txt");
        let incoming = make_op(peer2, 1, OpType::Add, "file.txt");

        let conflict = Conflict::new(PathBuf::from("file.txt"), base, incoming);
        let resolver = ConflictFile::new();

        // Non-Add conflicts fall back to last-write-wins
        // base (ts=100) > incoming (ts=1)
        assert_eq!(resolver.resolve(&conflict, &peer1), Resolution::UseBase);
    }
}
