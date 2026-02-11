# Daemon State for FUSE Mounts

**Status:** Planned
**Track:** Local
**Depends on:** Ticket 1 (FUSE filesystem)

## Objective

Add SQLite persistence for FUSE mount configurations and a `MountManager` struct to hold running mount state in the daemon.

## Implementation Steps

### 1. SQLite migration

**Create:** Migration for `fuse_mounts` table

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

### 2. Mount queries module

**Create:** `crates/daemon/src/daemon/database/mount_queries.rs`

- `create_mount(mount: &FuseMount) -> Result<()>`
- `get_mount(mount_id: &str) -> Result<Option<FuseMount>>`
- `list_mounts() -> Result<Vec<FuseMount>>`
- `update_mount(mount: &FuseMount) -> Result<()>`
- `delete_mount(mount_id: &str) -> Result<()>`
- `update_mount_status(mount_id: &str, status: &str, pid: Option<i32>, error: Option<&str>) -> Result<()>`
- `get_auto_mount_list() -> Result<Vec<FuseMount>>`

### 3. MountManager struct

**Create:** Within `crates/daemon/src/fuse/` or `crates/daemon/src/daemon/`

Behind `#[cfg(feature = "fuse")]`:

```rust
pub struct MountManager {
    /// Running mount processes: mount_id → child process handle
    running: HashMap<String, MountProcess>,
    /// Database pool for persistence
    db: SqlitePool,
}
```

### 4. Wire into ServiceState

**Modify:** `crates/daemon/src/daemon/state.rs`

- Add `#[cfg(feature = "fuse")] mount_manager: MountManager` field
- Add accessor method

## Files Summary

| Action | Path |
|--------|------|
| Create | Migration SQL for `fuse_mounts` table |
| Create | `crates/daemon/src/daemon/database/mount_queries.rs` |
| Modify | `crates/daemon/src/daemon/database/mod.rs` (add mount_queries module) |
| Modify | `crates/daemon/src/daemon/state.rs` (add MountManager) |

## Acceptance Criteria

- [ ] `fuse_mounts` table created by migration
- [ ] CRUD operations on mount configs via mount_queries
- [ ] `MountManager` tracks running mount processes
- [ ] `MountManager` integrated into `ServiceState` behind feature flag
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings
