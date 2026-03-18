# FUSE: mv does not work (within mount or into mount)

**Status:** Open
**Priority:** High
**Category:** Bugs
**Auto:** false

## Problem

`mv` fails in two scenarios involving FUSE mounts:

### 1. mv within a mount → Input/output error

```
cd jax          # mounted directory
mv readme.md test
mv: rename readme.md to test: Input/output error
```

The `rename()` handler in `jax_fs.rs:1167` calls `mount_guard.mv()` directly, which fails and returns `libc::EIO`. The error path at line 1265 logs the underlying cause but the user only sees "Input/output error".

### 2. mv from outside into a mount → xattr error

```
mv readme.md jax/
mv: jax/readme.md: unable to move extended attributes and ACL from readme.md: Operation not permitted
```

When `mv` crosses filesystem boundaries (host → FUSE), macOS falls back to copy+delete. After copying the file data, it tries to preserve extended attributes by calling `setxattr`. Our FUSE handler at `jax_fs.rs:1271` returns `ENOTSUP`, which macOS surfaces as "Operation not permitted".

## Root Causes

1. **rename within mount**: The `mount.mv()` call is failing — needs investigation. Could be a path resolution issue, a locking issue with the double `block_on` pattern (inode read lock at line 1199 then mount write lock at line 1239), or a bug in the mv logic itself. The error is swallowed into `EIO` without surfacing to the user.

2. **mv into mount (cross-filesystem)**: `setxattr` returns `ENOTSUP`. On macOS, `mv` treats xattr preservation failure as a hard error when moving files. The fix is to silently accept (and discard) xattr writes — jax doesn't use xattrs, and refusing them blocks a common filesystem operation.

## Implementation Steps

### 1. Fix setxattr to silently succeed

**File**: `crates/daemon/src/fuse/jax_fs.rs` (line 1271)

Change `setxattr` to reply with `reply.ok()` instead of `reply.error(libc::ENOTSUP)`. Extended attributes are not meaningful in jax's content-addressed storage model, but rejecting them breaks macOS `mv` and `cp -p`.

### 2. Debug and fix rename within mount

**File**: `crates/daemon/src/fuse/jax_fs.rs` (line 1167)

Add better error logging to surface the actual `mount.mv()` failure reason. Investigate:
- Whether the `block_on` pattern causes a deadlock (inode read lock held while acquiring mount write lock)
- Whether path format mismatches cause `mv()` to fail (leading slash conventions)
- Whether the mount state is stale when rename is called

### 3. Verify api_mv endpoint works independently

Test the daemon HTTP API `mv` endpoint directly (via curl or CLI) to confirm the mount-level `mv()` logic works outside the FUSE context. This isolates whether the bug is in the FUSE handler or the mount logic.

## Files to Modify

- `crates/daemon/src/fuse/jax_fs.rs` — fix `setxattr` reply, improve rename error handling

## Verification

1. `mv readme.md test` within a mounted bucket succeeds
2. `mv readme.md jax/` from outside into a mounted bucket succeeds
3. Renamed/moved files persist after unmount and remount
4. `cargo test && cargo clippy`
