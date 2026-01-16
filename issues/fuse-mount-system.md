# FUSE Mount Management System

**Status:** In Progress (implementation complete, needs integration)
**Epic:** None (standalone feature)
**Dependencies:** None

## Reference Implementation

Branch: [`amiller68/fs-over-blobstore-v1`](https://github.com/jax-protocol/jax-buckets/tree/amiller68/fs-over-blobstore-v1)
Commit: `e4b2bbf`

Complete FUSE implementation with mount persistence, REST API, and CLI commands.

## Objective

Allow users to mount remote buckets as local filesystems using FUSE, with persistent configuration, lifecycle management, and integration with the P2P sync system.

## Architecture Overview

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   Local FS      │     │   FUSE Process   │     │    Daemon       │
│   Operations    │────▶│   (jax_fs.rs)    │────▶│   HTTP API      │
│   (read/write)  │     │                  │     │                 │
└─────────────────┘     └──────────────────┘     └─────────────────┘
                                                        │
                                                        ▼
                                                ┌─────────────────┐
                                                │     Mount       │
                                                │   (bucket.rs)   │
                                                └─────────────────┘
                                                        │
                                                        ▼
                                                ┌─────────────────┐
                                                │   Blob Store    │
                                                │   + PathOpLog   │
                                                │     (CRDT)      │
                                                └─────────────────┘
```

### Key Components

| Component | File | Purpose |
|-----------|------|---------|
| `JaxFs` | `crates/app/src/fuse/jax_fs.rs` | FUSE filesystem using `fuser` crate |
| `Mount` | `crates/common/src/mount/bucket.rs` | Blob store file/directory operations |
| `PathOpLog` | `crates/common/src/mount/path_ops.rs` | CRDT for conflict-free sync |
| `MountManager` | `crates/app/src/daemon/mount_manager.rs` | FUSE process lifecycle |
| Daemon HTTP API | `crates/app/src/daemon/http_server/api/v0/mounts/` | REST endpoints |

## Design Decision: HTTP-Based Approach

The FUSE process communicates with the daemon via HTTP rather than direct `Mount` access because:

1. **Daemon as coordination point**: All modifications go through daemon for sync
2. **CRDT integration**: `PathOpLog` needs to be wired into sync protocol
3. **Process isolation**: FUSE runs as separate process for simpler lifecycle

## FUSE Operations Implemented

| Operation | Purpose | Implementation |
|-----------|---------|----------------|
| `lookup` | Resolve path to inode | `path_to_inode` HashMap |
| `getattr` | Get file/directory attributes | Returns size, permissions, timestamps |
| `readdir` | List directory contents | Uses `Mount::ls()` |
| `read` | Read file contents | Uses `Mount::read()` with LRU cache |
| `write` | Write to file | Buffers writes, flushes on `flush`/`release` |
| `create` | Create new file | Adds to `pending_creates`, creates on first write |
| `mkdir` | Create directory | Uses `Mount::mkdir()` |
| `unlink` | Delete file | Uses `Mount::rm()` |
| `rmdir` | Delete directory | Uses `Mount::rm()` |
| `rename` | Move/rename file | Uses `Mount::mv()` |

## Bug Fixes Implemented

### 1. First-Write Failure

**Problem**: Files created via FUSE failed on first write (daemon didn't know about them).

**Solution**: Synchronous flush on first write to pending files:

```rust
if is_pending && offset == 0 {
    // First write to pending file - flush synchronously
    rt.block_on(self.api_add(p, data))?;
    self.pending_creates.write().unwrap().remove(p);
}
```

### 2. Path Duplication Bug

**Problem**: Creating `/test.md` resulted in `/test.md/test.md`.

**Solution**: Send parent directory as `mount_path`, not full path.

### 3. macOS Resource Fork Filtering

**Problem**: macOS Finder creates `._*` files that cluttered filesystem.

**Solution**: Filter in `lookup()` and `create()`:

```rust
if name_str.starts_with("._") {
    reply.error(ENOENT);
    return;
}
```

## Mount Persistence System

### Database Schema

```sql
CREATE TABLE fuse_mounts (
    mount_id TEXT PRIMARY KEY,
    bucket_id TEXT NOT NULL,
    mount_point TEXT NOT NULL UNIQUE,
    enabled INTEGER NOT NULL DEFAULT 1,
    auto_mount INTEGER NOT NULL DEFAULT 0,
    read_only INTEGER NOT NULL DEFAULT 0,
    cache_size_mb INTEGER NOT NULL DEFAULT 100,
    cache_ttl_secs INTEGER NOT NULL DEFAULT 60,
    pid INTEGER,
    status TEXT NOT NULL DEFAULT 'stopped',
    error_message TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

### Mount Status Values

| Status | Meaning |
|--------|---------|
| `Stopped` | Mount not running |
| `Running` | FUSE process active |
| `Error` | Mount failed with error |

## REST API Endpoints

All under `/api/v0/mounts/`:

| Method | Path | Description |
|--------|------|-------------|
| POST | `/` | Create mount configuration |
| GET | `/` | List all mounts |
| GET | `/:id` | Get single mount |
| PATCH | `/:id` | Update mount config |
| DELETE | `/:id` | Delete mount config |
| POST | `/:id/start` | Start mount (spawn FUSE) |
| POST | `/:id/stop` | Stop mount (kill FUSE) |

## CLI Commands

All under `jax mount`:

| Command | Description |
|---------|-------------|
| `jax mount list [--json]` | List all configured mounts |
| `jax mount add <bucket> <path> [--auto-mount] [--read-only]` | Add mount configuration |
| `jax mount remove <id> [-f]` | Remove mount configuration |
| `jax mount start <id>` | Start a mount |
| `jax mount stop <id>` | Stop a mount |
| `jax mount set <id> [options]` | Update mount settings |

## Files Created/Modified

### New Files

| File | Description |
|------|-------------|
| `crates/app/src/fuse/jax_fs.rs` | FUSE filesystem implementation |
| `crates/app/src/fuse/mod.rs` | Module exports |
| `crates/app/src/daemon/mount_manager.rs` | Process lifecycle management |
| `crates/app/src/daemon/database/mount_queries.rs` | SQLite queries |
| `crates/app/src/daemon/http_server/api/v0/mounts/*.rs` | REST endpoints |
| `crates/app/src/ops/mount/*.rs` | CLI commands |
| `migrations/20260105000000_create_fuse_mounts.up.sql` | Database migration |

### Modified Files

| File | Changes |
|------|---------|
| `crates/app/src/daemon/state.rs` | Add MountManager |
| `crates/app/src/daemon/process/mod.rs` | Auto-mount and graceful shutdown |
| `crates/app/src/ops/mod.rs` | Add mount command enum |
| `crates/app/Cargo.toml` | Add `fuser` dependency |

## Daemon Integration

### Auto-Mount on Startup

```rust
// In daemon process startup
tokio::spawn(async move {
    mount_manager.start_auto_mounts().await;
});
```

### Graceful Shutdown

```rust
// On shutdown signal
state.mount_manager().stop_all_mounts().await;
```

### Platform-Specific Unmount

```rust
#[cfg(target_os = "macos")]
Command::new("umount").arg(path).status()?;

#[cfg(target_os = "linux")]
Command::new("fusermount").args(["-u", path]).status()?;
```

## Acceptance Criteria

### Completed
- [x] FUSE filesystem with all basic operations
- [x] Inode management (bidirectional mapping)
- [x] HTTP-based daemon communication
- [x] Bug fixes (first-write, path duplication, resource forks)
- [x] SQLite mount persistence
- [x] MountManager process lifecycle
- [x] REST API endpoints (7 endpoints)
- [x] CLI commands (6 commands)
- [x] Auto-mount on daemon startup
- [x] Graceful shutdown

### Remaining
- [ ] Health monitoring for crashed mounts
- [ ] Write buffering improvements
- [ ] Block-level caching for large files
- [ ] Direct Mount access for reads (reduce latency)
- [ ] Cache invalidation on sync events

## Verification

```bash
# Add a mount
jax mount add my-bucket ~/mounts/my-bucket --auto-mount

# Start the mount
jax mount start <mount-id>

# Verify filesystem works
ls ~/mounts/my-bucket
echo "test" > ~/mounts/my-bucket/test.txt
cat ~/mounts/my-bucket/test.txt

# Stop the mount
jax mount stop <mount-id>

# Remove configuration
jax mount remove <mount-id>
```

## Future Considerations

1. **Direct Mount Access**: Pass `Mount` directly for reads, HTTP only for writes
2. **Health Monitoring**: Periodic checks, auto-restart crashed mounts
3. **Write Buffering**: Coalesce writes, background flush, write-ahead logging
4. **Caching**: Block-level for large files, predictive prefetching
