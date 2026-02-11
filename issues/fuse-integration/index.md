# FUSE Integration

**Status:** Planned
**Reference:** `amiller68/fs-over-blobstore-v1` branch

## Summary

Mount buckets as local filesystems using FUSE. All FUSE code lives in the daemon crate behind a `fuse` cargo feature flag. The FUSE process communicates with the daemon via its REST API, keeping the daemon as the single coordination point for sync and CRDT operations.

## Architecture

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  Local FS    │────▶│  JaxFs       │────▶│  Daemon      │
│  Operations  │     │  (FUSE)      │     │  REST API    │
└──────────────┘     └──────────────┘     └──────────────┘
                                                │
                                                ▼
                                          ┌──────────────┐
                                          │  Mount       │
                                          │  + BlobStore │
                                          │  + PathOpLog │
                                          └──────────────┘

jax daemon (with fuse feature)
├── FUSE module at crates/daemon/src/fuse/
├── MountManager in daemon state
├── REST API endpoints at /api/v0/mounts/
├── CLI commands: jax mount list|add|remove|start|stop|set
└── SQLite fuse_mounts table for persistence
```

## Design Decisions

- **In-crate, not separate crate**: FUSE module lives in `crates/daemon/` behind a `fuse` feature flag rather than a standalone `crates/fuse/` crate. Simplifies dependency management and daemon integration.
- **HTTP-based communication**: FUSE process talks to daemon via HTTP. Daemon stays the single coordination point for sync, CRDT, and blob operations.
- **Process isolation**: FUSE runs as a spawned process for simpler lifecycle management and crash isolation.

## Tickets

| # | Ticket | Status | Description |
|---|--------|--------|-------------|
| 1 | [FUSE filesystem](./1-fuse-filesystem.md) | Planned | JaxFs implementation, REST endpoints, CLI commands |
| 2 | [Daemon state](./2-daemon-state.md) | Planned | SQLite persistence, mount queries, MountManager |
| 3 | [Daemon lifecycle](./3-daemon-lifecycle.md) | Planned | Start/stop, auto-mount, graceful shutdown |
| 4 | [Desktop integration](./4-desktop-integration.md) | Planned | Tauri IPC, SolidJS mounts page |

## Execution Order

```
Ticket 1: FUSE filesystem (JaxFs + REST API + CLI)
    │
    ▼
Ticket 2: Daemon state (SQLite + MountManager)
    │
    ▼
Ticket 3: Daemon lifecycle (auto-mount, shutdown, health)
    │
    ▼
Ticket 4: Desktop integration (IPC + UI)
```

Tickets 1-2 can partially overlap. Ticket 3 depends on both. Ticket 4 can start once the REST API (ticket 1) is stable.

## Reference Branch

The `amiller68/fs-over-blobstore-v1` branch contains a working prototype with all FUSE operations, mount persistence, and daemon integration. Key files to reference:

| File (in reference branch) | Purpose |
|---------------------------|---------|
| `crates/app/src/fuse/jax_fs.rs` | FUSE filesystem implementation |
| `crates/app/src/fuse/mod.rs` | Module exports |
| `crates/app/src/daemon/mount_manager.rs` | Process lifecycle management |
| `crates/app/src/daemon/database/mount_queries.rs` | SQLite queries |
| `crates/app/src/daemon/http_server/api/v0/mounts/` | REST endpoints |
| `crates/app/src/ops/mount/` | CLI commands |
