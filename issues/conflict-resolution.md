# Extensible Conflict Resolution for PathOpLog

**Status:** Planned
**Epic:** None (standalone feature)
**Dependencies:** PR #32 (path operation CRDT - merged)

## Reference Implementation

Branch: [`amiller68/conflict-resolution`](https://github.com/jax-protocol/jax-buckets/tree/amiller68/conflict-resolution)

This branch contains a working implementation with full test coverage (32 tests). Use as reference when implementing.

## Objective

Add a pluggable conflict resolution system to `PathOpLog` that allows different applications to define custom merge behavior when concurrent operations from different peers affect the same path.

## Background

The current CRDT implementation uses a hardcoded "last write wins" strategy:
- All operations are added to the log during merge
- Resolution happens at read time via `resolve_path()` / `resolve_all()`
- The operation with the highest `OpId` (Lamport timestamp + peer ID tie-breaker) wins

Some applications need different behaviors:
1. **Fork strategy**: Preserve both versions by renaming one (e.g., `document.txt` → `document@a1b2c3d4.txt`)
2. **Base wins strategy**: Reject incoming changes on conflict, prioritizing stability over freshness

## Implementation Steps

### 1. Create conflict.rs module

Create `crates/common/src/mount/conflict.rs` with:

```rust
/// Describes a conflict between two operations on the same path
pub struct Conflict<'a> {
    pub path: &'a Path,
    pub base_op: &'a PathOperation,      // Operation in current state
    pub incoming_op: &'a PathOperation,  // Operation being merged in
}

/// Result of conflict resolution
pub enum Resolution {
    KeepBase,           // Discard incoming operation
    AcceptIncoming,     // Add incoming operation as-is
    Fork { forked_path: PathBuf },  // Add with modified path
}

/// Trait for pluggable conflict resolution
pub trait ConflictResolver: Send + Sync {
    fn resolve(&self, conflict: &Conflict) -> Resolution;
}

/// Tracks merge statistics
pub struct MergeResult {
    pub added: usize,    // Operations added without conflict
    pub rejected: usize, // Operations discarded (KeepBase)
    pub forked: usize,   // Operations added with modified path
}
```

### 2. Implement built-in strategies

| Strategy | Behavior | Use Case |
|----------|----------|----------|
| `LastWriteWins` | Always accept incoming, resolve at read time | Default CRDT behavior |
| `ForkOnConflict` | Create `<name>@<8-char-peer-hash>.<ext>` | Collaborative editing |
| `BaseWins` | Always reject conflicting incoming ops | Stable deployments |

### 3. Add merge_with_resolver to PathOpLog

In `crates/common/src/mount/path_ops.rs`:

```rust
impl PathOpLog {
    /// Existing merge (unchanged)
    pub fn merge(&mut self, other: &PathOpLog) -> usize;

    /// New: merge with custom conflict resolution
    pub fn merge_with_resolver<R: ConflictResolver>(
        &mut self,
        other: &PathOpLog,
        resolver: &R,
    ) -> MergeResult;
}
```

### 4. Add helper functions

- `find_concurrent_op()` - Find operation in base log concurrent with incoming
- `happens_before()` - Determine causal precedence between operations

### 5. Export types from mod.rs

Re-export from `crates/common/src/mount/mod.rs`:
- `BaseWins`, `Conflict`, `ConflictResolver`, `ForkOnConflict`, `LastWriteWins`, `MergeResult`, `Resolution`

## Files to Modify/Create

### New Files

| File | Description |
|------|-------------|
| `crates/common/src/mount/conflict.rs` | ConflictResolver trait, Resolution enum, built-in strategies |

### Modified Files

| File | Changes |
|------|---------|
| `crates/common/src/mount/path_ops.rs` | Add `merge_with_resolver()`, `find_concurrent_op()`, `happens_before()` helpers |
| `crates/common/src/mount/mod.rs` | Re-export conflict types |

## Implementation Complications

### Conflict Detection

Two operations are "concurrent" (and thus conflicting) if:
- They affect the same path
- Neither causally precedes the other

Causal precedence (`happens_before`) is determined by:
```rust
fn happens_before(op1: &OpId, op2: &OpId) -> bool {
    op1.peer_id == op2.peer_id && op1.timestamp < op2.timestamp
}
```

Operations from **different peers** with the **same timestamp** are concurrent and require resolution.

### Fork Path Generation

The `ForkOnConflict` strategy needs to generate unique paths:
- Extract file stem and extension
- Append `@<short-hash>` before extension
- Use first 8 characters of incoming peer's public key as the hash

Edge cases:
- Files without extensions: `README` → `README@a1b2c3d4`
- Hidden files: `.gitignore` → `.gitignore@a1b2c3d4` (stem is empty, extension is `gitignore`)
- Nested paths: `dir/file.txt` → `dir/file@a1b2c3d4.txt`

### LastWriteWins Semantics

Key insight: `LastWriteWins` should **always accept** incoming operations because:
- The "last write wins" resolution happens at read time via `resolve_path()`
- Rejecting operations during merge would lose history
- This matches the original CRDT merge behavior

```rust
impl ConflictResolver for LastWriteWins {
    fn resolve(&self, _conflict: &Conflict) -> Resolution {
        Resolution::AcceptIncoming  // Always accept, resolve later
    }
}
```

### Clock Updates

The local Lamport clock must be updated during merge:
```rust
if id.timestamp >= self.local_clock {
    self.local_clock = id.timestamp + 1;
}
```

### Operation Metadata Preservation

When forking, preserve original: `OpId`, `op_type`, `content_link`, `is_dir`. Only modify `path`.

## Acceptance Criteria

- [ ] `ConflictResolver` trait defined with `resolve()` method
- [ ] `Resolution` enum with `KeepBase`, `AcceptIncoming`, `Fork` variants
- [ ] `MergeResult` struct tracks added/rejected/forked counts
- [ ] `LastWriteWins` strategy always accepts incoming
- [ ] `ForkOnConflict` strategy generates correct forked paths
- [ ] `BaseWins` strategy always rejects conflicting incoming
- [ ] `merge_with_resolver()` method on `PathOpLog`
- [ ] All types exported from `mount` module
- [ ] Unit tests for each resolver
- [ ] Integration tests for merge scenarios
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings

## Verification

```bash
# Run all tests
cargo test -p jax-common

# Run conflict-specific tests
cargo test -p jax-common conflict
cargo test -p jax-common merge_with

# Check for warnings
cargo clippy -p jax-common
```

## Test Coverage Required

### Unit Tests (conflict.rs)
- Each resolver returns expected Resolution
- Fork generates valid paths (with/without extension)
- MergeResult tracking

### Integration Tests (path_ops.rs)
- `merge_with_resolver` with each strategy
- Multiple concurrent conflicts in single merge
- Different operation types (Add, Remove, Mkdir, Mv)
- Same-peer operations (causal, not concurrent)
- Idempotency: merging twice has no effect
- Commutativity: merge order doesn't affect final state (for LastWriteWins)
- Empty log merges
- Clock advancement after merge
- Three-way merges

## Open Questions

1. **Should Fork create a synthetic operation for the base version too?**
   - Currently only the incoming op gets a forked path
   - The base op keeps its original path

2. **How should the app layer choose a resolver?**
   - Per-bucket configuration?
   - Per-sync-operation parameter?
   - Global app setting?

3. **Should rejected operations be logged somewhere?**
   - Currently silently discarded
   - Might want audit trail for debugging

4. **What about cascading conflicts?**
   - If forking creates `file@abc.txt` and that path already exists, what happens?

## Usage Example

```rust
use jax_common::mount::{PathOpLog, ForkOnConflict, BaseWins, LastWriteWins};

// Fork strategy - collaborative editing
let result = log.merge_with_resolver(&incoming, &ForkOnConflict);
if result.forked > 0 {
    println!("Created {} conflict copies", result.forked);
}

// Base wins - stable deployment
let result = log.merge_with_resolver(&incoming, &BaseWins);
println!("Rejected {} conflicting changes", result.rejected);

// Custom resolver
struct MyResolver;
impl ConflictResolver for MyResolver {
    fn resolve(&self, conflict: &Conflict) -> Resolution {
        if matches!(conflict.incoming_op.op_type, OpType::Remove) {
            Resolution::KeepBase  // Never accept remote deletes
        } else {
            Resolution::AcceptIncoming
        }
    }
}
```

## Estimated Effort

- **Implementation**: ~300 lines of new code
- **Tests**: ~400 lines of test code
