# FUSE Filesystem

**Status:** Planned
**Track:** Local
**Reference:** `amiller68/fs-over-blobstore-v1` branch

## Objective

Implement `JaxFs` FUSE filesystem, REST API mount endpoints, and CLI mount commands — all behind a `fuse` cargo feature flag in the daemon crate.

## Implementation Steps

### 1. FUSE module

**Create:** `crates/daemon/src/fuse/mod.rs` — Module exports
**Create:** `crates/daemon/src/fuse/jax_fs.rs` — FUSE filesystem

`JaxFs` implements `fuser::Filesystem`:

| Operation | Purpose | Backend |
|-----------|---------|---------|
| `lookup` | Resolve path to inode | `path_to_inode` HashMap |
| `getattr` | Get file/dir attributes | Returns size, permissions, timestamps |
| `readdir` | List directory contents | `Mount::ls()` via HTTP |
| `read` | Read file contents | `Mount::read()` via HTTP with LRU cache |
| `write` | Write to file | Buffers writes, flushes on `flush`/`release` |
| `create` | Create new file | Adds to `pending_creates`, creates on first write |
| `mkdir` | Create directory | `Mount::mkdir()` via HTTP |
| `unlink` | Delete file | `Mount::rm()` via HTTP |
| `rmdir` | Delete directory | `Mount::rm()` via HTTP |
| `rename` | Move/rename | `Mount::mv()` via HTTP |

Key implementation details:
- **Inode mapping**: Bidirectional inode ↔ path mapping, root inode = 1
- **Write buffering**: Buffer writes in memory, flush on `release`
- **First-write sync**: Synchronous flush on first write to pending files (prevents first-write failure bug)
- **macOS resource forks**: Filter `._*` files in `lookup()` and `create()`

### 2. REST API endpoints

**Create:** `crates/daemon/src/daemon/http_server/api/v0/mounts/`

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/v0/mounts/` | Create mount config |
| GET | `/api/v0/mounts/` | List all mounts |
| GET | `/api/v0/mounts/:id` | Get mount |
| PATCH | `/api/v0/mounts/:id` | Update mount config |
| DELETE | `/api/v0/mounts/:id` | Delete mount config |
| POST | `/api/v0/mounts/:id/start` | Start mount |
| POST | `/api/v0/mounts/:id/stop` | Stop mount |

### 3. CLI commands

**Create:** `crates/daemon/src/ops/mount.rs`

| Command | Description |
|---------|-------------|
| `jax mount list [--json]` | List all configured mounts |
| `jax mount add <bucket> <path> [--auto-mount] [--read-only]` | Add mount config |
| `jax mount remove <id> [-f]` | Remove mount config |
| `jax mount start <id>` | Start mount |
| `jax mount stop <id>` | Stop mount |
| `jax mount set <id> [options]` | Update mount settings |

### 4. Feature flag setup

**Modify:** `crates/daemon/Cargo.toml`
- Add `fuser` dependency behind `fuse` feature
- Add `fuse` feature flag

**Modify:** `crates/daemon/src/lib.rs`
- `#[cfg(feature = "fuse")] pub mod fuse;`

## Files Summary

| Action | Path |
|--------|------|
| Create | `crates/daemon/src/fuse/mod.rs` |
| Create | `crates/daemon/src/fuse/jax_fs.rs` |
| Create | `crates/daemon/src/daemon/http_server/api/v0/mounts/` (module with CRUD + start/stop) |
| Create | `crates/daemon/src/ops/mount.rs` |
| Modify | `crates/daemon/Cargo.toml` (add `fuser`, `fuse` feature) |
| Modify | `crates/daemon/src/lib.rs` (conditional fuse module) |
| Modify | `crates/daemon/src/main.rs` (add mount subcommand) |

## Acceptance Criteria

- [ ] `JaxFs` implements all 10 FUSE operations listed above
- [ ] macOS resource fork filtering works
- [ ] Write buffering with sync-on-first-write works
- [ ] 7 REST endpoints functional
- [ ] 6 CLI commands functional
- [ ] Compiles without `fuse` feature (no `fuser` dependency pulled in)
- [ ] `cargo build --features fuse` compiles
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings
