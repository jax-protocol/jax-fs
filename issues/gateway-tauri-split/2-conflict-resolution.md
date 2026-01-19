# Conflict Resolution

**Status:** Done
**Track:** Common
**Reference:** `amiller68/conflict-resolution` branch, `issues/conflict-resolution.md`

## Objective

Add pluggable conflict resolution to PathOpLog for handling concurrent edits from different peers.

## Implementation Steps

1. Add `conflict.rs` module to `jax-common`
2. Implement `ConflictResolver` trait
3. Implement built-in strategies (LastWriteWins, ForkOnConflict, BaseWins)
4. Add `merge_with_resolver()` to PathOpLog
5. Wire into sync protocol

## Files to Create

| File | Description |
|------|-------------|
| `crates/common/src/mount/conflict.rs` | ConflictResolver trait, strategies |

## Files to Modify

| File | Changes |
|------|---------|
| `crates/common/src/mount/path_ops.rs` | Add merge_with_resolver() |
| `crates/common/src/mount/mod.rs` | Export conflict types |
| `crates/common/src/peer/sync/` | Use resolver during sync |

## Acceptance Criteria

- [x] ConflictResolver trait defined
- [x] Three built-in strategies implemented
- [x] merge_with_resolver() works correctly
- [ ] Sync uses configurable resolver (infrastructure ready; sync doesn't use PathOpLog yet)
- [x] `cargo test` passes (82 tests total, 23 new conflict resolution tests)
- [x] `cargo clippy` has no warnings

## Implementation Notes

Created `conflict.rs` module with:

- **`ConflictResolver` trait** - Interface for custom resolution strategies
- **`Conflict` struct** - Represents a conflict between base and incoming operations
- **`Resolution` enum** - `UseBase`, `UseIncoming`, `KeepBoth`, `SkipBoth`
- **`MergeResult` struct** - Result with operations added, resolved/unresolved conflicts

Built-in strategies:
- **`LastWriteWins`** - Higher timestamp wins (default CRDT behavior)
- **`BaseWins`** - Local operations always win
- **`ForkOnConflict`** - Keep both, return unresolved for manual resolution

Public exports from `mount` module:
- `ConflictResolver`, `Conflict`, `Resolution`, `MergeResult`, `ResolvedConflict`
- `LastWriteWins`, `BaseWins`, `ForkOnConflict`
- `operations_conflict`, `conflicts_with_mv_source`
