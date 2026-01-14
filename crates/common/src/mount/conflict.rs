//! Conflict resolution strategies for PathOpLog merges
//!
//! This module provides pluggable conflict resolution for when concurrent operations
//! from different peers affect the same path. Apps can choose from built-in strategies
//! or implement custom resolution logic.
//!
//! # Built-in Strategies
//!
//! - [`LastWriteWins`]: Higher OpId wins (default, deterministic)
//! - [`ForkOnConflict`]: Keep both versions by renaming incoming to `<name>@<hash>`
//! - [`BaseWins`]: Always keep base, discard incoming changes

use std::path::{Path, PathBuf};

use super::path_ops::PathOperation;

/// Describes a conflict between two operations on the same path
#[derive(Debug)]
pub struct Conflict<'a> {
    /// The path where the conflict occurred
    pub path: &'a Path,
    /// Operation in the current (base) state
    pub base_op: &'a PathOperation,
    /// Operation being merged in
    pub incoming_op: &'a PathOperation,
}

/// Result of conflict resolution
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Keep the base operation (discard incoming)
    KeepBase,
    /// Accept the incoming operation (replace base)
    AcceptIncoming,
    /// Fork: keep both by renaming the incoming content
    Fork {
        /// New path for the incoming content
        forked_path: PathBuf,
    },
}

/// Trait for pluggable conflict resolution strategies
///
/// Implement this trait to define custom conflict resolution behavior.
/// The resolver is called when merging operations that affect the same path
/// and are concurrent (neither causally precedes the other).
pub trait ConflictResolver: Send + Sync {
    /// Resolve a conflict between two concurrent operations
    ///
    /// # Arguments
    /// * `conflict` - Information about the conflicting operations
    ///
    /// # Returns
    /// A [`Resolution`] indicating how to handle the conflict
    fn resolve(&self, conflict: &Conflict) -> Resolution;
}

/// Default strategy: accept all operations, resolve at read time
///
/// This preserves the original CRDT behavior where all operations are kept
/// in the log. The "last write wins" resolution happens automatically when
/// calling `resolve_path()` or `resolve_all()` - the operation with the
/// highest OpId wins.
///
/// This strategy maintains full operation history, enabling future
/// reconciliation and debugging of conflicts.
#[derive(Debug, Clone, Copy, Default)]
pub struct LastWriteWins;

impl ConflictResolver for LastWriteWins {
    fn resolve(&self, _conflict: &Conflict) -> Resolution {
        // Always accept incoming - resolution happens at read time
        Resolution::AcceptIncoming
    }
}

/// Fork strategy: create `<name>@<short-hash>` for conflicts
///
/// Preserves both versions by renaming the incoming file. The forked name
/// is constructed as `<stem>@<8-char-peer-hash>.<ext>`.
///
/// Example: `document.txt` becomes `document@a1b2c3d4.txt`
#[derive(Debug, Clone, Copy, Default)]
pub struct ForkOnConflict;

impl ConflictResolver for ForkOnConflict {
    fn resolve(&self, conflict: &Conflict) -> Resolution {
        // Use first 8 chars of the incoming peer's public key as identifier
        let peer_str = conflict.incoming_op.id.peer_id.to_string();
        let short_hash = &peer_str[..8.min(peer_str.len())];

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

        let forked_name = format!("{}@{}{}", stem, short_hash, ext);
        let forked_path = conflict.path.with_file_name(forked_name);

        Resolution::Fork { forked_path }
    }
}

/// Base wins strategy: always keep base, discard incoming
///
/// This strategy prioritizes stability over freshness. Any conflicting
/// changes from other peers are silently discarded.
#[derive(Debug, Clone, Copy, Default)]
pub struct BaseWins;

impl ConflictResolver for BaseWins {
    fn resolve(&self, _conflict: &Conflict) -> Resolution {
        Resolution::KeepBase
    }
}

/// Result of a merge operation with conflict resolution
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergeResult {
    /// Number of operations added without conflict
    pub added: usize,
    /// Number of operations rejected (KeepBase resolution)
    pub rejected: usize,
    /// Number of operations forked to new paths
    pub forked: usize,
}

impl MergeResult {
    /// Total number of operations processed
    pub fn total(&self) -> usize {
        self.added + self.rejected + self.forked
    }

    /// Check if any conflicts were detected
    pub fn had_conflicts(&self) -> bool {
        self.rejected > 0 || self.forked > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{PublicKey, SecretKey};

    fn make_peer_id(seed: u8) -> PublicKey {
        let mut seed_bytes = [0u8; 32];
        seed_bytes[0] = seed;
        let secret = SecretKey::from(seed_bytes);
        secret.public()
    }

    fn make_op(peer_id: PublicKey, timestamp: u64, path: &str) -> PathOperation {
        use super::super::path_ops::{OpId, OpType};

        PathOperation {
            id: OpId { timestamp, peer_id },
            op_type: OpType::Add,
            path: PathBuf::from(path),
            content_link: None,
            is_dir: false,
        }
    }

    #[test]
    fn test_last_write_wins_always_accepts() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        // Test with incoming having higher timestamp
        let base_op = make_op(peer1, 1, "file.txt");
        let incoming_op = make_op(peer2, 2, "file.txt");

        let conflict = Conflict {
            path: Path::new("file.txt"),
            base_op: &base_op,
            incoming_op: &incoming_op,
        };

        let resolver = LastWriteWins;
        // LastWriteWins always accepts - resolution happens at read time
        assert_eq!(resolver.resolve(&conflict), Resolution::AcceptIncoming);

        // Test with incoming having lower timestamp - still accepts
        let base_op2 = make_op(peer1, 2, "file.txt");
        let incoming_op2 = make_op(peer2, 1, "file.txt");

        let conflict2 = Conflict {
            path: Path::new("file.txt"),
            base_op: &base_op2,
            incoming_op: &incoming_op2,
        };

        // Still accepts - the "last write wins" part happens via resolve_path()
        assert_eq!(resolver.resolve(&conflict2), Resolution::AcceptIncoming);
    }

    #[test]
    fn test_fork_on_conflict() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let base_op = make_op(peer1, 1, "document.txt");
        let incoming_op = make_op(peer2, 1, "document.txt");

        let conflict = Conflict {
            path: Path::new("document.txt"),
            base_op: &base_op,
            incoming_op: &incoming_op,
        };

        let resolver = ForkOnConflict;
        let resolution = resolver.resolve(&conflict);

        match resolution {
            Resolution::Fork { forked_path } => {
                let name = forked_path.file_name().unwrap().to_string_lossy();
                assert!(name.starts_with("document@"));
                assert!(name.ends_with(".txt"));
                // Should have 8 char hash between @ and .txt
                let middle = &name[9..name.len() - 4];
                assert_eq!(middle.len(), 8);
            }
            _ => panic!("Expected Fork resolution"),
        }
    }

    #[test]
    fn test_fork_on_conflict_no_extension() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let base_op = make_op(peer1, 1, "README");
        let incoming_op = make_op(peer2, 1, "README");

        let conflict = Conflict {
            path: Path::new("README"),
            base_op: &base_op,
            incoming_op: &incoming_op,
        };

        let resolver = ForkOnConflict;
        let resolution = resolver.resolve(&conflict);

        match resolution {
            Resolution::Fork { forked_path } => {
                let name = forked_path.file_name().unwrap().to_string_lossy();
                assert!(name.starts_with("README@"));
                assert!(!name.contains('.'));
            }
            _ => panic!("Expected Fork resolution"),
        }
    }

    #[test]
    fn test_base_wins() {
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let base_op = make_op(peer1, 1, "file.txt");
        let incoming_op = make_op(peer2, 100, "file.txt"); // Higher timestamp doesn't matter

        let conflict = Conflict {
            path: Path::new("file.txt"),
            base_op: &base_op,
            incoming_op: &incoming_op,
        };

        let resolver = BaseWins;
        assert_eq!(resolver.resolve(&conflict), Resolution::KeepBase);
    }

    #[test]
    fn test_merge_result() {
        let mut result = MergeResult::default();
        assert_eq!(result.total(), 0);
        assert!(!result.had_conflicts());

        result.added = 5;
        assert_eq!(result.total(), 5);
        assert!(!result.had_conflicts());

        result.rejected = 2;
        assert_eq!(result.total(), 7);
        assert!(result.had_conflicts());

        result.forked = 1;
        assert_eq!(result.total(), 8);
        assert!(result.had_conflicts());
    }
}
