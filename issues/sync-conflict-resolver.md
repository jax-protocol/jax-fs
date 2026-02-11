# Wire Conflict Resolution into Sync Protocol

**Status:** Planned
**Follow-up from:** Gateway-Tauri-Split ticket 2 (Conflict resolution)

## Objective

Wire the pluggable `ConflictResolver` infrastructure into the P2P sync protocol so that peers use configurable resolution strategies when merging PathOpLogs during sync.

## Background

The `ConflictResolver` trait and built-in strategies (`LastWriteWins`, `BaseWins`, `ForkOnConflict`) are implemented in `jax-common`. The `merge_with_resolver()` method on `PathOpLog` works correctly with 23+ tests. However, the sync protocol doesn't use `PathOpLog` merging yet — it needs to be wired in.

## Implementation Steps

### 1. Add resolver config to sync

- Allow configuring which `ConflictResolver` strategy to use per bucket or globally
- Default to `LastWriteWins`

### 2. Wire into peer sync

**Files:** `crates/common/src/peer/sync/`

- During P2P sync, when merging incoming operations, use `merge_with_resolver()` instead of direct append
- Handle `MergeResult` — report unresolved conflicts to the user

### 3. Surface conflicts in UI

- Expose unresolved conflicts via REST API
- Show in Tauri desktop app (e.g., conflict list with manual resolution)

## Acceptance Criteria

- [ ] Sync uses `merge_with_resolver()` for PathOpLog merges
- [ ] Default strategy is `LastWriteWins`
- [ ] Users can configure resolver per bucket
- [ ] Unresolved conflicts (from `ForkOnConflict`) are surfaced
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings
