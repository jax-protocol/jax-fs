# SQLite + Object Storage Blob Store

**Status:** Complete (iroh-blobs Store backend implemented)
**Epic:** None (standalone feature)
**Dependencies:** None

## Reference Implementation

Branch: [`amiller68/sqlite-minio-blobs`](https://github.com/jax-protocol/jax-buckets/tree/amiller68/sqlite-minio-blobs)

Prototype crate with SQLite metadata + object storage backend. Compiles and passes clippy.

## Objective

Create a custom blob store backend that separates metadata storage (SQLite) from blob data storage (S3/MinIO/local filesystem), enabling cloud-native deployments where blob data lives in object storage rather than on local disk.

## What Was Built

### New Crate: `crates/object-store/`

A standalone crate (`jax-object-store`) providing:
- SQLite for metadata (hash, size, state, timestamps)
- Object storage for blob data (via `object_store` crate)

```
crates/object-store/
├── Cargo.toml
├── migrations/
│   └── 20251223000000_create_blobs_store.sql
└── src/
    ├── lib.rs            # Public exports
    ├── object_store.rs   # Public ObjectStore API + internal BlobStore
    ├── database.rs       # SQLite pool + migrations
    ├── storage.rs        # Object storage wrapper + ObjectStoreConfig
    ├── actor.rs          # ObjectStoreActor for iroh-blobs proto::Request
    └── error.rs          # Error types
```

### App Config Integration

Added `BlobStoreConfig` enum to `crates/app/src/state.rs`:

```rust
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlobStoreConfig {
    Legacy,                              // existing iroh-blobs FsStore
    Filesystem { path: Option<PathBuf> }, // jax-object-store + local fs
    S3 { endpoint, access_key, secret_key, bucket, region, db_path },
}
```

### Dev Tooling

- `bin/dev minio` - Start MinIO container for S3 testing
- `bin/dev blob-stores` - Run gateways with different blob store backends

## Design Decisions

### 1. Separate Crate with Own Database

The blob store uses its own SQLite database, separate from the log provider database:
- Distinct responsibilities (blob metadata vs bucket logs)
- Independent migration paths
- Can use in-memory SQLite for blob metadata (recoverable from object storage)
- Cleaner module boundaries

### 2. `object_store` Crate over `aws-sdk-s3`

Uses the `object_store` crate instead of AWS SDK:
- Unified interface for S3/MinIO/GCS/Azure/local filesystem
- Simpler API for basic operations (put/get/list/delete)
- Well-maintained by Apache Arrow project
- Easy to swap backends without code changes

### 3. SQLite as Metadata Cache

SQLite stores only metadata, not blob data:
- Hash (BLAKE3, base32-encoded)
- Size in bytes
- State (complete/partial)
- Has outboard flag (for BAO trees > 16KB)
- Timestamps

SQLite can be treated as a cache rebuildable from object storage via `recover_from_storage()`.

### 4. Backward Compatible Config

The `blob_store` field in `AppConfig` uses `#[serde(default)]` with `Legacy` as the default variant:
- Existing config.toml files without `blob_store` continue to work
- No breaking changes for existing nodes
- Explicit opt-in to new storage backends

### 5. Object Storage Path Structure

```
{bucket}/blobs/{hash}/data      # blob content
{bucket}/blobs/{hash}/outboard  # BAO outboard (if size > 16KB)
```

Flat structure with hash-based keys enables:
- Easy recovery by listing objects
- No nested directories to traverse
- Direct access by hash

## Current State

### Working

- [x] Crate compiles and passes clippy
- [x] SQLite migrations via `sqlx::migrate!()` macro
- [x] Config parsing with backward compatibility
- [x] Legacy mode uses existing iroh-blobs FsStore
- [x] CLI flags for blob store selection (`--blob-store`, `--s3-*`)
- [x] Multiple constructors:
  - `BlobStore::new()` - file-based SQLite + S3
  - `BlobStore::in_memory()` - in-memory SQLite + S3
  - `BlobStore::new_local()` - SQLite + local filesystem
  - `BlobStore::new_ephemeral()` - fully in-memory for tests
- [x] Unit tests (10 tests + 1 doctest passing)

### Integrated

The new `BlobStore` is now **fully wired into `iroh_blobs::BlobsProtocol`** via ObjectStore:
- `BlobStoreConfig::Legacy` → uses `BlobsStore::legacy_fs()`
- `BlobStoreConfig::Filesystem` → uses `BlobsStore::fs()` with SQLite + local filesystem
- `BlobStoreConfig::S3` → uses `BlobsStore::s3()` with SQLite + S3/MinIO

## Implementation Details

### 1. Trait Bridge (Complete)

Implemented via `ObjectStoreActor` in `crates/object-store/src/actor.rs`:
- Handles all ~20 `proto::Request` command variants
- Maps operations to SQLite + Object Storage backend

### 2. Protocol Integration (Complete)

Wired into `BlobsProtocol` via `ObjectStore`:
- `crates/object-store/src/object_store.rs` - ObjectStore wrapper with `Into<iroh_blobs::api::Store>`
- `crates/common/src/peer/blobs_store.rs` - Added `fs()`, `memory()`, `s3()`, `from_store()` constructors
- `crates/app/src/daemon/blobs/setup.rs` - Uses BlobsStore constructors for S3/Filesystem configs

### 3. Verified Streaming (Basic Implementation)

BAO operations implemented in actor.rs:
- `ImportBao` - Receives BAO chunks, verifies hash, stores data
- `ExportBao` - Creates outboard and sends leaf chunks
- Full BAO tree traversal is simplified (sends all data as leaves)

### 4. Partial Blob Support (Future Enhancement)

Not yet implemented:
- Track partial state in SQLite
- Resume from object storage

## Files Changed

### New Files

| File | Description |
|------|-------------|
| `crates/object-store/*` | Entire new crate (`jax-object-store`) |
| `crates/object-store/src/actor.rs` | ObjectStoreActor handling proto::Request commands |
| `crates/object-store/src/object_store.rs` | ObjectStore wrapper for iroh-blobs Store API |

### Modified Files

| File | Changes |
|------|---------|
| `Cargo.toml` | Workspace member |
| `crates/app/Cargo.toml` | Added object-store dependency |
| `crates/app/src/state.rs` | Added `BlobStoreConfig` enum |
| `crates/app/src/daemon/config.rs` | Replaced `node_blobs_store_path` with `blob_store` + `jax_dir` |
| `crates/app/src/daemon/blobs/setup.rs` | Uses BlobsStore constructors for S3/Filesystem configs |
| `crates/app/src/ops/daemon.rs` | Added CLI flags and `build_blob_store_config()` |
| `crates/common/src/peer/blobs_store.rs` | Added `fs()`, `memory()`, `s3()`, `from_store()` constructors |
| `crates/object-store/Cargo.toml` | Added irpc, bao-tree, range-collections dependencies |
| `bin/dev` | Added `minio` and `blob-stores` commands |

## CLI Usage

Blob store is configured at **init time**, not daemon time:

```bash
# Legacy (default)
jax init

# Local filesystem
jax init --blob-store filesystem --blobs-path /data/blobs

# S3/MinIO (credentials in URL)
jax init --blob-store s3 --s3-url s3://minioadmin:minioadmin@localhost:9000/jax-blobs
```

## Config File Format

After init, config.toml contains:

```toml
# Legacy (default)
[blob_store]
type = "legacy"

# Filesystem
[blob_store]
type = "filesystem"
path = "/data/blobs"

# S3
[blob_store]
type = "s3"
url = "s3://minioadmin:minioadmin@localhost:9000/jax-blobs"
```

## Recovery from Object Storage

If SQLite metadata is lost, it can be rebuilt:

```rust
let store = BlobStore::in_memory(s3_config).await?;
let stats = store.recover_from_storage().await?;
// stats.complete_blobs, stats.partial_blobs, etc.
```

Scans object storage, verifies blob integrity, and repopulates SQLite. Tags would be lost (only stored in SQLite currently).

## Acceptance Criteria

- [x] Trait bridge implements iroh-blobs store traits
- [x] Protocol integration wires BlobStore into BlobsProtocol
- [x] Filesystem config uses new BlobStore (not fallback)
- [x] S3 config uses new BlobStore (not fallback)
- [x] BAO verified streaming works (basic implementation)
- [ ] Partial blob support implemented (future enhancement)
- [x] `cargo test` passes with new backends
- [x] `cargo clippy` has no warnings
- [x] Backward compatibility maintained (legacy config works)

## Verification

```bash
# Start MinIO for S3 testing
./bin/dev minio

# Run tests
cargo test -p jax-object-store

# Init with S3 config, then run daemon
jax init --blob-store s3 --s3-url s3://minioadmin:minioadmin@localhost:9000/jax-blobs
jax daemon --gateway
```
