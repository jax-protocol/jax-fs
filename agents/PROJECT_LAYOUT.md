# Project Layout

Quick guide to finding your way around the jax-bucket workspace.

## Crates

### `crates/daemon` - CLI & Daemon (lib + bin)

The main binary (`jax-daemon`) and library (`jax_daemon`). The library exports daemon functionality for embedding (used by Tauri). The binary handles CLI commands and runs the headless HTTP daemon (REST API + gateway).

**Key areas:**

- `src/lib.rs` - Library entry point, re-exports service modules and state
- `src/main.rs` - Binary entry point, CLI parsing
- `src/http_server/` - HTTP servers (API + gateway)
  - `api/v0/bucket/` - REST API handlers (add, cat, create, delete, shares, etc.)
  - `api/v0/mounts/` - FUSE mount REST API (create, list, get, update, delete, start, stop)
  - `api/client/` - API client for CLI commands and FUSE operations
  - `html/gateway/` - Gateway HTML handlers for published content
  - `gateway_index.rs` - Gateway index page (lists published buckets)
- `src/database/` - SQLite storage and bucket log provider
  - `mount_queries.rs` - FUSE mount persistence (CRUD, status updates)
- `src/blobs/` - Blob store setup and configuration
- `src/fuse/` - FUSE filesystem integration (behind `fuse` feature flag)
  - `mod.rs` - Module exports
  - `jax_fs.rs` - FUSE filesystem implementation using fuser
  - `mount_manager.rs` - Mount lifecycle management (start, stop, auto-mount)
  - `inode_table.rs` - Bidirectional inode ↔ path mapping
  - `cache.rs` - LRU content cache with TTL
  - `sync_events.rs` - Sync event types for cache invalidation
- `src/process/` - Service lifecycle (start, spawn, shutdown, auto-mount)
- `src/service_config.rs` - Service configuration (ports, paths, blob store)
- `src/service_state.rs` - Runtime state (database, peer, mount_manager)
- `src/state.rs` - App state (jax directory paths, config file)
- `src/cli/` - CLI-specific code (not exported by library)
  - `args.rs` - CLI argument parsing
  - `op.rs` - Op trait and command_enum macro
  - `ops/` - CLI command implementations (bucket, daemon, health, init, mount, version)
    - `bucket/shares/` - Share management subcommands (create, ls)
    - `mount/` - Mount CLI commands (list, add, remove, start, stop, set) — gated behind `fuse` feature

### `crates/common` - Core Library

Shared library (`jax-common`) with crypto, storage, and peer protocol.

**Key areas:**

- `src/crypto/` - Keys, encryption, secret sharing
  - `keys.rs` - Ed25519/X25519 keypairs
  - `secret.rs` - ChaCha20-Poly1305 encryption
  - `secret_share.rs` - X25519 key exchange for sharing
- `src/mount/` - Virtual filesystem
  - `manifest.rs` - Bucket metadata, shares, principals
  - `mount_inner.rs` - File operations (add, rm, mkdir, mv)
  - `node.rs` - File/directory tree nodes
  - `path_ops.rs` - PathOpLog CRDT for tracking filesystem changes
  - `conflict/` - Conflict resolution for PathOpLog merges
    - `mod.rs` - ConflictResolver trait, helpers, exports
    - `types.rs` - Conflict, Resolution, MergeResult types
    - `last_write_wins.rs` - LastWriteWins resolver (default CRDT)
    - `base_wins.rs` - BaseWins resolver (local wins)
    - `fork_on_conflict.rs` - ForkOnConflict resolver (keep both)
    - `conflict_file.rs` - ConflictFile resolver (rename incoming)
- `src/peer/` - P2P networking
  - `peer_inner.rs` - Peer state and mount operations
  - `blobs_store.rs` - Content-addressed blob storage (iroh-blobs)
  - `protocol/` - Wire protocol messages
  - `sync/` - Sync jobs (download, ping, sync bucket)
- `src/bucket_log/` - Append-only log for bucket history

### `crates/object-store` - Blob Storage

SQLite + object storage backend for blob data (`jax-object-store`).

**Key areas:**

- `src/object_store.rs` - Public ObjectStore API + internal BlobStore (put, get, delete, list, recover)
- `src/database.rs` - SQLite metadata storage (hash, size, state)
- `src/storage.rs` - S3/MinIO/local/memory storage wrapper + ObjectStoreConfig
- `src/actor.rs` - iroh-blobs proto::Request command handler (ObjectStoreActor)
- `src/error.rs` - Error types
- `migrations/` - SQLite schema

#### Integration Tests

- `tests/` - Integration tests for mount operations
- `tests/common/mod.rs` - Shared test utilities (`setup_test_env()`)

### `crates/desktop` - Desktop App

Tauri 2.0 desktop application (`jax-desktop`) with SolidJS frontend. Embeds the daemon in-process and exposes IPC commands that access `ServiceState` directly (no HTTP proxying for most operations). Released via GitHub Actions (not cargo publish).

**Key areas:**

- `src-tauri/src/lib.rs` - Tauri entry point, daemon lifecycle management
- `src-tauri/src/commands/bucket.rs` - Bucket IPC commands (list, ls, cat, add, mkdir, delete, history, shares)
- `src-tauri/src/commands/daemon.rs` - Daemon status and config IPC commands
- `src-tauri/src/commands/mount.rs` - FUSE mount IPC commands (list, create, start, stop, delete, simplified mount/unmount)
- `src-tauri/src/tray.rs` - System tray setup (Open, Status, Quit)
- `src-tauri/capabilities/default.json` - Tauri permission capabilities
- `src-tauri/tauri.conf.json` - Tauri configuration
- `src/` - SolidJS frontend source
  - `App.tsx` - Root component with router and sidebar layout
  - `lib/api.ts` - IPC wrapper functions (TypeScript bindings for all commands including mounts)
  - `pages/Home.tsx` - Node status dashboard
  - `pages/Buckets.tsx` - Bucket list, creation, and one-click mount/unmount buttons
  - `pages/Mounts.tsx` - Advanced mount management (manual mount point selection)
  - `pages/Explorer.tsx` - File explorer with breadcrumbs, upload, mkdir, delete, share
  - `pages/Viewer.tsx` - File viewer (text, markdown, images, video, audio)
  - `pages/Editor.tsx` - Text file editor with save
  - `pages/History.tsx` - Bucket version history with navigation to past versions
  - `pages/Settings.tsx` - Auto-launch toggle, theme switcher, local config paths
  - `components/SharePanel.tsx` - Slide-in panel for peer sharing

## Other Directories

- `agents/` - Documentation for AI agents (you're reading one)
  - `API.md` - HTTP API reference
  - `DEBUG.md` - Debugging workflow guide
- `bin/` - Shell scripts for build, check, dev, test
  - `dev` - Development environment entry point (`./bin/dev`)
  - `dev_/` - Dev environment modules and config
    - `nodes.toml` - Node definitions (ports, blob stores, nicknames)
    - `fixtures.toml` - Initial data fixtures
    - `*.sh` - Modules (api, logs, fixtures, nodes, config)
  - `db` - SQLite database helper
  - `minio` - MinIO local server for S3-compatible blob storage testing
  - `utils/` - Utility scripts for demos and development
- `.github/workflows/` - CI and release automation
