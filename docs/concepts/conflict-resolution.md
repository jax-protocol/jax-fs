# Conflict Resolution

This document describes JaxBucket's pluggable conflict resolution system for handling concurrent edits during peer synchronization.

## Overview

When two peers edit the same file path concurrently (without seeing each other's changes), a conflict occurs. JaxBucket provides a pluggable resolution system that lets applications choose how to handle these conflicts.

**Location**: `crates/common/src/mount/conflict.rs`

## Conflict Detection

Two operations are **concurrent** (conflicting) if:
- They affect the same path
- They come from different peers
- Neither causally precedes the other

```text
Peer A (timestamp 5)          Peer B (timestamp 5)
        │                             │
        ▼                             ▼
   Add "notes.txt"              Add "notes.txt"
   (content: "Hello")           (content: "World")
        │                             │
        └──────────┬──────────────────┘
                   │
                   ▼
              CONFLICT!
        (same path, same timestamp,
         different peers)
```

**Causal precedence** is determined by:
```rust
fn happens_before(op1: &OpId, op2: &OpId) -> bool {
    op1.peer_id == op2.peer_id && op1.timestamp < op2.timestamp
}
```

Operations from the **same peer** are never concurrent (Lamport clocks ensure ordering).

## Resolution Strategies

JaxBucket provides four built-in strategies:

| Strategy | Behavior | Use Case |
|----------|----------|----------|
| `LastWriteWins` | Higher timestamp/peer ID wins | Default CRDT behavior |
| `BaseWins` | Local operation always wins | Stable deployments |
| `ForkOnConflict` | Keep both, return for manual resolution | Audit/review workflows |
| `ConflictFile` | Rename incoming to `file@<hash>.ext` | Collaborative editing |

### LastWriteWins (Default)

Both operations are added to the log; resolution happens at read time via `resolve_path()`. The operation with the higher `OpId` (Lamport timestamp, then peer ID tie-breaker) wins.

```rust
impl ConflictResolver for LastWriteWins {
    fn resolve(&self, conflict: &Conflict, _local_peer: &PublicKey) -> Resolution {
        Resolution::UseIncoming  // Always accept, resolve later
    }
}
```

### ConflictFile (Recommended for Sync)

Creates a "conflict copy" for the incoming version, preserving both:

```text
Before conflict:
  notes.txt (Alice's version)

After sync with ConflictFile resolver:
  notes.txt          (Alice's version - unchanged)
  notes.txt@a1b2c3d4 (Bob's version - renamed with content hash)
```

The hash suffix comes from the incoming file's content link, ensuring:
- Same content = same conflict filename (idempotent)
- Different content = different conflict filename (no overwrites)

```rust
impl ConflictResolver for ConflictFile {
    fn resolve(&self, conflict: &Conflict, _local_peer: &PublicKey) -> Resolution {
        match (&conflict.base.op_type, &conflict.incoming.op_type) {
            (OpType::Add, OpType::Add) => {
                // Both are adds - create conflict file for incoming
                let hash = conflict.incoming.content_link.hash().to_string();
                let version = &hash[..8];  // First 8 chars
                let new_path = Self::conflict_path(&conflict.incoming.path, version);
                Resolution::RenameIncoming { new_path }
            }
            _ => {
                // Other conflicts: fall back to last-write-wins
                Resolution::UseIncoming
            }
        }
    }
}
```

### BaseWins

Always keeps the local version, rejecting incoming changes on conflict:

```rust
impl ConflictResolver for BaseWins {
    fn resolve(&self, _conflict: &Conflict, _local_peer: &PublicKey) -> Resolution {
        Resolution::UseBase
    }
}
```

### ForkOnConflict

Keeps both operations in the log and returns them for manual resolution:

```rust
impl ConflictResolver for ForkOnConflict {
    fn resolve(&self, _conflict: &Conflict, _local_peer: &PublicKey) -> Resolution {
        Resolution::KeepBoth
    }
}
```

## Resolution Enum

```rust
pub enum Resolution {
    UseBase,                          // Keep local, discard incoming
    UseIncoming,                      // Replace with incoming
    KeepBoth,                         // Add both to log (fork)
    SkipBoth,                         // Discard both
    RenameIncoming { new_path },      // Add incoming at different path
}
```

## Usage

### Basic Merge with Resolver

```rust
use jax_common::mount::{PathOpLog, ConflictFile, MergeResult};

let mut local_log = PathOpLog::new(local_peer);
let incoming_log = PathOpLog::new(remote_peer);

// Merge with conflict file strategy
let result: MergeResult = local_log.merge_with_resolver(
    &incoming_log,
    &ConflictFile::new(),
    &local_peer,
);

println!("Added: {}", result.operations_added);
println!("Conflicts resolved: {}", result.conflicts_resolved.len());
```

### Custom Resolver

```rust
struct NeverDeleteResolver;

impl ConflictResolver for NeverDeleteResolver {
    fn resolve(&self, conflict: &Conflict, _local_peer: &PublicKey) -> Resolution {
        // Never accept remote deletes
        if matches!(conflict.incoming.op_type, OpType::Remove) {
            Resolution::UseBase
        } else {
            Resolution::UseIncoming
        }
    }
}
```

## Merge Result

The `MergeResult` struct tracks what happened during merge:

```rust
pub struct MergeResult {
    pub operations_added: usize,           // Ops added without conflict
    pub conflicts_resolved: Vec<ResolvedConflict>,  // Auto-resolved conflicts
    pub unresolved_conflicts: Vec<Conflict>,        // For ForkOnConflict
}

pub struct ResolvedConflict {
    pub conflict: Conflict,
    pub resolution: Resolution,
}
```

## Conflict File Naming

For `ConflictFile` strategy, filenames are generated as:

| Original | Content Hash | Result |
|----------|--------------|--------|
| `report.txt` | `abc123de...` | `report.txt@abc123de` |
| `Makefile` | `deadbeef...` | `Makefile@deadbeef` |
| `src/lib.rs` | `cafebabe...` | `src/lib.rs@cafebabe` |

The hash length is configurable (default: 8 characters):

```rust
let resolver = ConflictFile::with_hash_length(16);  // Use 16 chars
```

## Integration with Sync

During bucket synchronization, the resolver is applied when merging incoming PathOpLog entries:

```text
1. Peer A receives manifest chain from Peer B
2. For each manifest, extract PathOpLog operations
3. Merge into local log using configured resolver
4. Conflict files created as needed
5. User sees both versions and can manually reconcile
```

The sync protocol itself doesn't choose the resolver - that's an application-level decision.

## Mount-Level Merge API

For higher-level operations, the `Mount` struct provides methods to merge divergent chains:

### Finding Common Ancestor

```rust
// Find where two mount chains diverged
let ancestor: Option<Link> = alice.find_common_ancestor(&bob, &blobs).await?;
```

Walks both manifest chains backwards via `previous` links and returns the first common link.

### Collecting Operations Since Ancestor

```rust
// Get all ops from current version back to (not including) ancestor
let ops: PathOpLog = alice.collect_ops_since(ancestor.as_ref(), &blobs).await?;
```

Traverses the manifest chain, loading and merging each version's `ops_log`.

### Merging Two Mounts

```rust
// Merge Bob's changes into Alice with conflict resolution
let resolver = ConflictFile::new();
let (result, new_link) = alice.merge_from(&bob, &resolver, &blobs).await?;

// Check what happened
println!("Conflicts: {}", result.total_conflicts());
for resolved in &result.conflicts_resolved {
    println!("  {} -> {:?}", resolved.conflict.path.display(), resolved.resolution);
}
```

The `merge_from` method:
1. Finds the common ancestor
2. Collects ops from both chains since that point
3. Merges using the resolver
4. Saves the result as a new version

## Design Decisions

### Why Content Hash Instead of Timestamp?

Earlier designs used timestamps (`file@1234567890.txt`), but content hashes are better:

1. **Idempotent**: Re-syncing the same conflict produces the same filename
2. **Meaningful**: Users can compare hashes to identify identical content
3. **Stable**: Doesn't depend on clock synchronization between peers

### Why Only Add vs Add Creates Conflict Files?

Other conflict types (Add vs Remove, Remove vs Remove) are resolved with last-write-wins because:

1. **User intent is clear**: One peer wants the file, one doesn't
2. **No data loss risk**: The "losing" version is still in the log history
3. **Simpler UX**: Users don't see conflict files for deletions
