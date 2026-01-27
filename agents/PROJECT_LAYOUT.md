# Project Layout

Quick guide to finding your way around the jax-bucket workspace.

## Crates

### `crates/app` - CLI & Daemon

The main binary (`jax-bucket`). Handles CLI commands, runs the HTTP daemon, and serves the web UI.

**Key areas:**

- `src/main.rs` - Entry point, CLI parsing
- `src/daemon/` - HTTP server and database
  - `http_server/api/v0/bucket/` - REST API handlers (add, cat, create, delete, etc.)
  - `http_server/html/` - Web UI page handlers
  - `database/` - SQLite storage and bucket log provider
  - `blobs/` - Blob store setup and configuration
- `src/ops/` - CLI command implementations

#### Askama Templating

The web UI uses [Askama](https://github.com/djc/askama) for HTML templating.

- `templates/layouts/` - Base layouts (`base.html`, `explorer.html`)
- `templates/pages/` - Full page templates
  - `pages/buckets/` - Bucket explorer, file viewer, history, peers
  - `pages/gateway/` - Read-only gateway UI (explorer, viewer, identity page)
- `templates/components/` - Reusable UI components (cards, modals, sidebars)

Templates are compiled at build time. Handler structs derive `Template` and reference template files.

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
