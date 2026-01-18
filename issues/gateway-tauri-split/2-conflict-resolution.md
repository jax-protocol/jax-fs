# Conflict Resolution

**Status:** Planned
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

- [ ] ConflictResolver trait defined
- [ ] Three built-in strategies implemented
- [ ] merge_with_resolver() works correctly
- [ ] Sync uses configurable resolver
- [ ] `cargo test` passes (32 tests from reference branch)
- [ ] `cargo clippy` has no warnings
