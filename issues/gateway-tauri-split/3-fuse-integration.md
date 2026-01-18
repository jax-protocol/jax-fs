# FUSE Mount Integration

**Status:** Planned
**Track:** Local
**Reference:** `amiller68/fs-over-blobstore-v1` branch, `issues/fuse-mount-system.md`

## Objective

Add FUSE mount support for mounting buckets as local filesystems.

## Implementation Steps

1. Add `fuse/` module to app crate
2. Implement JaxFs FUSE filesystem
3. Implement MountManager for process lifecycle
4. Add SQLite mount persistence (migration)
5. Add `jax mount` CLI commands (add, remove, start, stop, list)
6. Add REST API endpoints for mount management
7. Add Askama UI for mount management (optional)

## Files to Create

| File | Description |
|------|-------------|
| `crates/app/src/fuse/mod.rs` | FUSE module |
| `crates/app/src/fuse/jax_fs.rs` | FUSE filesystem |
| `crates/app/src/daemon/mount_manager.rs` | Process lifecycle |
| `crates/app/src/ops/mount.rs` | CLI commands |
| `crates/app/src/daemon/http_server/api/v0/mounts/` | REST endpoints |
| Migration for fuse_mounts table |

## Files to Modify

| File | Changes |
|------|---------|
| `crates/app/Cargo.toml` | Add fuser dependency |
| `crates/app/src/main.rs` | Add mount subcommand |
| `crates/app/src/daemon/state.rs` | Add MountManager |

## Acceptance Criteria

- [ ] `jax mount add <bucket> <path>` creates mount config
- [ ] `jax mount start <id>` mounts bucket
- [ ] FUSE operations work (read, write, mkdir, rm)
- [ ] Mounts persist across daemon restarts
- [ ] Auto-mount on startup works
- [ ] Graceful unmount on shutdown
- [ ] macOS and Linux supported

## Verification

```bash
jax mount add my-bucket ~/mounts/my-bucket
jax mount start <mount-id>
ls ~/mounts/my-bucket
echo "test" > ~/mounts/my-bucket/test.txt
cat ~/mounts/my-bucket/test.txt
jax mount stop <mount-id>
```
