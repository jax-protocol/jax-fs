# SQLite + Object Storage Blob Store

**Status:** In Progress (prototype complete, integration pending)
**Epic:** None (standalone feature)
**Dependencies:** None

## Reference Implementation

Branch: [`amiller68/sqlite-minio-blobs`](https://github.com/jax-protocol/jax-buckets/tree/amiller68/sqlite-minio-blobs)

Prototype crate with SQLite metadata + object storage backend. Compiles and passes clippy.

## Objective

Create a custom blob store backend that separates metadata storage (SQLite) from blob data storage (S3/MinIO/local filesystem), enabling cloud-native deployments where blob data lives in object storage rather than on local disk.

## What Was Built

### New Crate: `crates/blobs-store/`

A standalone crate (`jax-blobs-store`) providing:
- SQLite for metadata (hash, size, state, timestamps)
- Object storage for blob data (via `object_store` crate)

```
crates/blobs-store/
├── Cargo.toml
├── migrations/
│   └── 20251223000000_create_blobs_store.sql
└── src/
    ├── lib.rs           # Public exports
    ├── store.rs         # Main BlobStore API
    ├── database.rs      # SQLite pool + migrations
    ├── object_store.rs  # Object storage wrapper
    └── error.rs         # Error types
```

### App Config Integration

Added `BlobStoreConfig` enum to `crates/app/src/state.rs`:

```rust
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlobStoreConfig {
    Legacy,                              // existing iroh-blobs FsStore
    Filesystem { path: Option<PathBuf> }, // jax-blobs-store + local fs
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

### Not Yet Integrated

The new `BlobStore` is **not wired into `iroh_blobs::BlobsProtocol`**. Currently:
- `BlobStoreConfig::Legacy` → uses existing `BlobsStore::fs()`
- `BlobStoreConfig::Filesystem` → falls back to `BlobsStore::fs()` with TODO
- `BlobStoreConfig::S3` → falls back to `BlobsStore::memory()` with warning

## Remaining Implementation Steps

### 1. Trait Bridge

Implement iroh-blobs store traits for `jax-blobs-store::BlobStore`:
- `iroh_blobs::store::Store` trait
- Read/write operations mapped to object storage

### 2. Protocol Integration

Wire into `BlobsProtocol` for network serving:
- Replace fallback code with actual BlobStore usage
- Handle both legacy and new backends seamlessly

### 3. Verified Streaming

Use BAO outboard for verified byte-range reads:
- Reconstruct BAO tree from outboard file
- Enable streaming verification during transfers

### 4. Partial Blob Support

Handle interrupted uploads/downloads:
- Track partial state in SQLite
- Resume from object storage

## Files Changed

### New Files

| File | Description |
|------|-------------|
| `crates/blobs-store/*` | Entire new crate |

### Modified Files

| File | Changes |
|------|---------|
| `Cargo.toml` | Workspace member |
| `crates/app/Cargo.toml` | Added blobs-store dependency |
| `crates/app/src/state.rs` | Added `BlobStoreConfig` enum |
| `crates/app/src/daemon/config.rs` | Replaced `node_blobs_store_path` with `blob_store` + `jax_dir` |
| `crates/app/src/daemon/state.rs` | Added `setup_blobs_store()` helper |
| `crates/app/src/ops/daemon.rs` | Added CLI flags and `build_blob_store_config()` |
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

- [ ] Trait bridge implements iroh-blobs store traits
- [ ] Protocol integration wires BlobStore into BlobsProtocol
- [ ] Filesystem config uses new BlobStore (not fallback)
- [ ] S3 config uses new BlobStore (not fallback)
- [ ] BAO verified streaming works
- [ ] Partial blob support implemented
- [x] `cargo test` passes with new backends
- [x] `cargo clippy` has no warnings
- [x] Backward compatibility maintained (legacy config works)

## Verification

```bash
# Start MinIO for S3 testing
./bin/dev minio

# Run tests
cargo test -p jax-blobs-store

# Init with S3 config, then run daemon
jax init --blob-store s3 --s3-url s3://minioadmin:minioadmin@localhost:9000/jax-blobs
jax daemon --gateway
```
