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
- `src/ops/` - CLI command implementations

#### Askama Templating

The web UI uses [Askama](https://github.com/djc/askama) for HTML templating.

- `templates/layouts/` - Base layouts (`base.html`, `explorer.html`)
- `templates/pages/` - Full page templates
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
- `src/peer/` - P2P networking
  - `peer_inner.rs` - Peer state and mount operations
  - `blobs_store.rs` - Content-addressed blob storage (iroh-blobs)
  - `protocol/` - Wire protocol messages
  - `sync/` - Sync jobs (download, ping, sync bucket)
- `src/bucket_log/` - Append-only log for bucket history

#### Integration Tests

- `tests/` - Integration tests for mount operations
- `tests/common/mod.rs` - Shared test utilities (`setup_test_env()`)

## Other Directories

- `agents/` - Documentation for AI agents (you're reading one)
- `bin/` - Shell scripts for build, check, dev, test
- `.github/workflows/` - CI and release automation
