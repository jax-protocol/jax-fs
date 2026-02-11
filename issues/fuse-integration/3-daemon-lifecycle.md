# Daemon Mount Lifecycle

**Status:** Planned
**Track:** Local
**Depends on:** Ticket 2 (Daemon state)

## Objective

Implement mount lifecycle management: starting/stopping FUSE processes, auto-mounting on daemon startup, graceful unmount on shutdown, and health monitoring.

## Implementation Steps

### 1. Start/stop mount operations

**Modify:** `MountManager`

- `start_mount(mount_id: &str) -> Result<()>` — Spawn FUSE process, update status to `Running`, store PID
- `stop_mount(mount_id: &str) -> Result<()>` — Kill FUSE process, platform-specific unmount, update status to `Stopped`
- `stop_all_mounts() -> Result<()>` — Stop all running mounts

### 2. Auto-mount on daemon startup

**Modify:** `crates/daemon/src/daemon/process/mod.rs`

On daemon startup (after database is ready):
```rust
#[cfg(feature = "fuse")]
tokio::spawn(async move {
    mount_manager.start_auto_mounts().await;
});
```

Query `fuse_mounts WHERE auto_mount = 1 AND enabled = 1` and start each.

### 3. Graceful unmount on shutdown

**Modify:** `crates/daemon/src/daemon/process/mod.rs`

Integrate with `ShutdownHandle`:
```rust
#[cfg(feature = "fuse")]
state.mount_manager().stop_all_mounts().await;
```

### 4. Platform-specific unmount

```rust
#[cfg(target_os = "macos")]
Command::new("umount").arg(&mount_point).status()?;

#[cfg(target_os = "linux")]
Command::new("fusermount").args(["-u", &mount_point]).status()?;
```

### 5. Health monitoring

- Track mount status: `Stopped`, `Running`, `Error`
- On FUSE process exit, update status to `Error` with message
- Expose status via REST API (`GET /api/v0/mounts/:id` includes status)

## Files Summary

| Action | Path |
|--------|------|
| Modify | `crates/daemon/src/fuse/` (MountManager lifecycle methods) |
| Modify | `crates/daemon/src/daemon/process/mod.rs` (auto-mount, shutdown) |

## Acceptance Criteria

- [ ] `jax mount start <id>` spawns FUSE process and updates status
- [ ] `jax mount stop <id>` kills process and runs platform unmount
- [ ] Mounts with `auto_mount = true` start on daemon startup
- [ ] All mounts stop gracefully on daemon shutdown
- [ ] Crashed mounts have `Error` status with error message
- [ ] macOS and Linux unmount commands work
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings

## Verification

```bash
# Build with fuse feature
cargo build --features fuse

# Start daemon, add auto-mount
jax mount add my-bucket ~/mounts/my-bucket --auto-mount
jax mount start <mount-id>

# Verify filesystem
ls ~/mounts/my-bucket
echo "test" > ~/mounts/my-bucket/test.txt
cat ~/mounts/my-bucket/test.txt

# Restart daemon — mount should auto-start
# Stop daemon — mount should unmount gracefully
```
