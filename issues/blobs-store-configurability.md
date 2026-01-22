# Blobs Store Configurability

**Status:** Planned

## Objective

Make the blobs store more configurable:
1. Allow separate paths for SQLite metadata DB vs object storage
2. Make max blob import size configurable
3. Rename `S3Store` to `ObjectStore` (more accurate naming)

## Background

Currently `S3Store::new_local(data_dir)` hardcodes the SQLite DB at `data_dir/blobs.db`. Some deployments may want:
- SQLite on fast local SSD, objects on slower/larger storage
- SQLite in a specific location for backup purposes
- Different max blob sizes (1GB default may be too small for video, too large for constrained nodes)

## Proposed Changes

### 1. Separate DB and Object Storage Paths

```rust
// Current
S3Store::new_local(data_dir: &Path)

// Proposed
ObjectStore::new_local(db_path: &Path, objects_path: &Path)
```

### 2. Configurable Max Import Size

```rust
// Current: hardcoded const
const MAX_IMPORT_SIZE: u64 = 1024 * 1024 * 1024; // 1GB

// Proposed: part of config
pub struct BlobStoreConfig {
    pub max_import_size: u64,  // default 1GB
    // ...
}
```

### 3. Rename S3Store → ObjectStore

The current name is misleading since it also supports local filesystem and memory backends.

## Files to Modify

| File | Changes |
|------|---------|
| `crates/blobs-store/src/iroh_store.rs` | Rename S3Store → ObjectStore, update constructors |
| `crates/blobs-store/src/actor.rs` | Make MAX_IMPORT_SIZE configurable |
| `crates/blobs-store/src/store.rs` | Update BlobStore constructors |
| `crates/app/src/daemon/blobs/setup.rs` | Update usage |
| `crates/common/src/peer/blobs_store.rs` | Update usage |

## Acceptance Criteria

- [ ] SQLite path configurable separately from object storage path
- [ ] Max import size configurable (with sensible default)
- [ ] S3Store renamed to ObjectStore
- [ ] Existing configs continue to work (backward compatible)
- [ ] `cargo test` passes
- [ ] `cargo clippy` clean
