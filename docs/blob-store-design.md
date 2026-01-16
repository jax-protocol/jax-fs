# SQLite + Object Storage Blob Store

**Branch:** `amiller68/sqlite-minio-blobs`

## Overview

This document describes a prototype implementation of a custom blob store backend that separates metadata storage (SQLite) from blob data storage (S3/MinIO/local filesystem). The goal is to enable cloud-native deployments where blob data lives in object storage rather than on local disk.

## What Was Built

### New Crate: `crates/blobs-store/`

A standalone crate (`jax-blobs-store`) that provides a blob store with:
- SQLite for metadata (hash, size, state, timestamps)
- Object storage for all blob data (via `object_store` crate)

```
crates/blobs-store/
├── Cargo.toml
├── migrations/
│   ├── 20251223000000_create_blobs_store.up.sql
│   └── 20251223000000_create_blobs_store.down.sql
└── src/
    ├── lib.rs           # Public exports
    ├── store.rs         # Main BlobStore API
    ├── database.rs      # SQLite pool + migrations
    ├── object_store.rs  # Object storage wrapper
    ├── bao_file.rs      # BAO tree reconstruction
    ├── entry_state.rs   # Blob state tracking
    └── import.rs        # Blob import with BLAKE3
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

- `bin/minio.sh` - MinIO container management (start/stop/status)
- `bin/utils` - Shared bash utilities

## Design Decisions

### 1. Separate Crate with Own Database

The blob store uses its own SQLite database, separate from the log provider database. This was chosen because:
- Distinct responsibilities (blob metadata vs bucket logs)
- Independent migration paths
- Can use in-memory SQLite for blob metadata (recoverable from object storage)
- Cleaner module boundaries

### 2. `object_store` Crate over `aws-sdk-s3`

Uses the `object_store` crate instead of AWS SDK because:
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

This means SQLite can be treated as a cache that's rebuildable from object storage via `recover_from_storage()`.

### 4. Backward Compatible Config

The `blob_store` field in `AppConfig` uses `#[serde(default)]` with `Legacy` as the default variant. This ensures:
- Existing config.toml files without `blob_store` continue to work
- No breaking changes for existing nodes
- Explicit opt-in to new storage backends

### 5. Object Storage Path Structure

Blobs are stored with paths like:
```
{bucket}/blobs/{hash}/data      # blob content
{bucket}/blobs/{hash}/outboard  # BAO outboard (if size > 16KB)
```

This flat structure with hash-based keys enables:
- Easy recovery by listing objects
- No nested directories to traverse
- Direct access by hash

## Current State

### Working

- Crate compiles and passes clippy
- SQLite migrations via `sqlx::migrate!()` macro
- Config parsing with backward compatibility
- Legacy mode uses existing iroh-blobs FsStore
- Multiple constructors for different use cases:
  - `BlobStore::new()` - file-based SQLite + S3
  - `BlobStore::in_memory()` - in-memory SQLite + S3
  - `BlobStore::new_local()` - SQLite + local filesystem
  - `BlobStore::new_ephemeral()` - fully in-memory for tests

### Not Yet Integrated

The new `BlobStore` is **not wired into `iroh_blobs::BlobsProtocol`**. Currently:
- `BlobStoreConfig::Legacy` → uses existing `BlobsStore::fs()`
- `BlobStoreConfig::Filesystem` → falls back to `BlobsStore::fs()` with TODO
- `BlobStoreConfig::S3` → falls back to `BlobsStore::memory()` with warning

### Required Changes for Full Integration

1. **Trait Bridge**: Implement iroh-blobs store traits for `jax-blobs-store::BlobStore`
2. **Protocol Integration**: Wire into `BlobsProtocol` for network serving
3. **Verified Streaming**: Use BAO outboard for verified byte-range reads
4. **Partial Blob Support**: Handle interrupted uploads/downloads

## Example Configuration

```toml
# Legacy (default - no config needed)
[blob_store]
type = "legacy"

# Local filesystem (no S3 required)
[blob_store]
type = "filesystem"
path = "/data/blobs"  # optional, defaults to {jax_dir}/blobs

# S3/MinIO
[blob_store]
type = "s3"
endpoint = "http://localhost:9000"
access_key = "minioadmin"
secret_key = "minioadmin"
bucket = "jax-blobs"
region = "us-east-1"        # optional
db_path = "/data/blobs.db"  # optional
```

## Files Changed

### New Files
- `crates/blobs-store/*` - entire new crate
- `bin/minio.sh` - MinIO dev script
- `bin/utils` - bash utilities
- `bin/config` - project config

### Modified Files
- `Cargo.toml` - workspace member
- `crates/app/Cargo.toml` - added blobs-store dependency
- `crates/app/src/state.rs` - added `BlobStoreConfig` enum
- `crates/app/src/daemon/config.rs` - replaced `node_blobs_store_path` with `blob_store` + `jax_dir`
- `crates/app/src/daemon/state.rs` - added `setup_blobs_store()` helper
- `crates/app/src/ops/daemon.rs` - pass blob_store config
- `crates/app/src/ops/init.rs` - include blob_store in AppConfig

## Recovery from Object Storage

If SQLite metadata is lost, it can be rebuilt:

```rust
let store = BlobStore::in_memory(s3_config).await?;
let stats = store.recover_from_storage().await?;
// stats.complete_blobs, stats.partial_blobs, etc.
```

This scans object storage, verifies blob integrity, and repopulates SQLite. Tags would be lost (only stored in SQLite currently).
