# Gateway SQLite + Object Storage Blob Store

**Status:** In Progress
**Track:** Gateway
**Reference:** `sqlite-blob-store` branch, `issues/sqlite-object-storage-blobs.md`

## Objective

Integrate SQLite + Object Storage blob backend into gateway for cloud-native deployments.

## Implementation Steps

1. ✅ Create `crates/blobs-store/` crate
2. ⬜ Implement iroh-blobs store trait bridge
3. ✅ Add blob store config to gateway command (S3 endpoint, bucket, credentials)
4. ⬜ Wire blob store into gateway's peer (currently falls back to legacy)

## Files Created

| File | Description |
|------|-------------|
| `crates/blobs-store/Cargo.toml` | New crate |
| `crates/blobs-store/src/lib.rs` | Public exports |
| `crates/blobs-store/src/store.rs` | Main BlobStore API |
| `crates/blobs-store/src/database.rs` | SQLite pool + migrations |
| `crates/blobs-store/src/object_store.rs` | S3/MinIO wrapper |
| `crates/blobs-store/src/error.rs` | Error types |

## Files Modified

| File | Changes |
|------|---------|
| `Cargo.toml` | Add workspace member |
| `crates/app/Cargo.toml` | Add blobs-store dependency |
| `crates/app/src/state.rs` | Add `BlobStoreConfig` enum |
| `crates/app/src/daemon/config.rs` | Replace `node_blobs_store_path` with `blob_store` + `jax_dir` |
| `crates/app/src/daemon/state.rs` | Add `setup_blobs_store()` helper |
| `crates/app/src/ops/daemon.rs` | Add blob store CLI flags (`--blob-store`, `--s3-*`) |
| `bin/dev` | Add `minio` and `blob-stores` commands |

## Acceptance Criteria

- [x] `crates/blobs-store` compiles independently
- [ ] Trait bridge implements iroh-blobs store traits
- [x] `jax init --blob-store s3 --s3-url ...` parses correctly
- [x] SQLite metadata can be rebuilt from object storage (`recover_from_storage()`)
- [x] `cargo test` passes
- [x] `cargo clippy` has no warnings

## Remaining Work

- Implement iroh-blobs store traits for `BlobStore`
- Replace fallback code in `setup_blobs_store()` with actual BlobStore usage
- Add BAO verified streaming support

## Verification

```bash
# Start MinIO
./bin/dev minio

# Init with S3 config, then run gateway
jax init --blob-store s3 --s3-url s3://minioadmin:minioadmin@localhost:9000/jax-blobs
jax daemon --gateway
```
