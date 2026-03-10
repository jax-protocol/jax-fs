# FUSE: route mutations through the daemon API

**Status:** In Review
**Priority:** High
**Category:** Bugs
**Auto:** true

## Objective

Refactor the FUSE filesystem to use the daemon's HTTP API for all mutations instead of calling `Mount` methods directly, so persistence is handled in one place.

## Background

The FUSE layer (`crates/daemon/src/fuse/jax_fs.rs`) currently holds a direct `Arc<RwLock<Mount>>` reference and calls `mount_guard.rm()`, `mount_guard.add()`, `mount_guard.mkdir()`, `mount_guard.mv()` etc. directly. After each mutation it must manually send a `SaveRequest` to persist changes — but this is easy to forget, and in fact `unlink`, `mkdir`, and `rename` are all missing the `SaveRequest`, causing deletions (and other changes) to be lost on remount.

### Reproduction (rm not persisted)

```
cd jax          # mounted directory
ls              # shows: test
rm test
ls              # empty — looks deleted
# stop and restart the mount
cd jax
ls              # shows: test — file is back
```

### Root cause

In `jax_fs.rs`, the `flush` handler (line ~804) and `mknod` handler (line ~389) send a `SaveRequest` after mutating — so writes and file creation persist. But `unlink` (line ~1047), `mkdir` (line ~980), and `rename` (line ~1147) do not send a `SaveRequest`, so those changes only exist in memory.

Rather than patching each handler individually, the cleaner fix is to route all FUSE mutations through the daemon's HTTP API, which already handles persistence as a single source of truth.

## Implementation Steps

1. Replace the direct `Mount` reference in `JaxFs` with an HTTP client pointing at the daemon API
2. Map FUSE mutation handlers to existing API endpoints:
   - `write`/`flush` -> `POST /api/v0/bucket/{id}/add`
   - `unlink`/`rmdir` -> `POST /api/v0/bucket/{id}/delete`
   - `mkdir` -> `POST /api/v0/bucket/{id}/mkdir` (or equivalent)
   - `rename` -> `POST /api/v0/bucket/{id}/mv`
   - `mknod` -> `POST /api/v0/bucket/{id}/add`
3. Keep direct `Mount` reads (ls, cat, getattr) via the existing reference for performance, or route those through the API as well if simplicity is preferred
4. Remove the `SaveRequest` / `save_tx` channel from `JaxFs` since the API handles persistence
5. Verify all API endpoints persist correctly (save manifest after mutation)

## Files to Modify

- `crates/daemon/src/fuse/jax_fs.rs` - Replace direct `Mount` mutation calls with HTTP API calls
- `crates/daemon/src/fuse/mod.rs` - Update `JaxFs` construction (swap `Mount` for API client)
- `crates/daemon/src/fuse/mount_manager.rs` - Adjust how FUSE filesystems are created
- `crates/daemon/src/fuse/sync_events.rs` - Potentially remove `SaveRequest` if no longer needed

## Acceptance Criteria

- [ ] `rm <file>` inside a FUSE mount persists after stop/start
- [ ] `mkdir <dir>` inside a FUSE mount persists after stop/start
- [ ] `mv <old> <new>` inside a FUSE mount persists after stop/start
- [ ] File writes still persist (no regression)
- [ ] All FUSE mutations go through the daemon API
- [ ] `SaveRequest` channel removed or unused by FUSE
- [ ] `cargo test` passes
- [ ] `cargo clippy` passes

## Verification

1. Mount a bucket, create a file, stop/start mount — file persists
2. Mount a bucket, `rm` a file, stop/start mount — file stays deleted
3. Mount a bucket, `mkdir`, stop/start mount — directory persists
4. Mount a bucket, `mv` a file, stop/start mount — rename persists
