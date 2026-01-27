# Blobs Store Configurability

**Status:** Planned
**Priority:** Low

## Objective

Make the blobs store more configurable:
1. Allow separate paths for SQLite metadata DB vs object storage
2. Make max blob import size configurable

## Background

Currently `ObjectStore::new_local(data_dir)` hardcodes the SQLite DB at `data_dir/blobs.db`. Some deployments may want:
- SQLite on fast local SSD, objects on slower/larger storage
- SQLite in a specific location for backup purposes
- Different max blob sizes (1GB default may be too small for video, too large for constrained nodes)

## Completed

### ~~Rename S3Store → ObjectStore~~

Done. The crate was renamed from `jax-blobs-store` to `jax-object-store` (`crates/object-store/`), and `S3Store` was renamed to `ObjectStore` in `crates/object-store/src/object_store.rs`.

## Proposed Changes

### 1. Separate DB and Object Storage Paths

```rust
// Current
ObjectStore::new_local(data_dir: &Path)

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

## Files to Modify

| File | Changes |
|------|---------|
| `crates/object-store/src/object_store.rs` | Update ObjectStore constructors for separate paths |
| `crates/object-store/src/actor.rs` | Make MAX_IMPORT_SIZE configurable |
| `crates/app/src/daemon/blobs/setup.rs` | Update usage |
| `crates/common/src/peer/blobs_store.rs` | Update usage |

## Acceptance Criteria

- [ ] SQLite path configurable separately from object storage path
- [ ] Max import size configurable (with sensible default)
- [x] S3Store renamed to ObjectStore
- [ ] Existing configs continue to work (backward compatible)
- [ ] `cargo test` passes
- [ ] `cargo clippy` clean
