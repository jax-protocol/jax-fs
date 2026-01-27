# Gateway SQLite + Object Storage Blob Store

**Status:** Complete
**Track:** Gateway
**Reference:** `sqlite-blob-store` branch, `issues/sqlite-object-storage-blobs.md`

## Objective

Integrate SQLite + Object Storage blob backend into gateway for cloud-native deployments.

## Implementation Steps

1. ✅ Create `crates/object-store/` crate
2. ✅ Implement iroh-blobs store trait bridge (ObjectStoreActor + ObjectStore)
3. ✅ Add blob store config to gateway command (S3 endpoint, bucket, credentials)
4. ✅ Wire blob store into gateway's peer (uses BlobsStore constructors for S3/Filesystem configs)

## Files Created

| File | Description |
|------|-------------|
| `crates/object-store/Cargo.toml` | New crate (`jax-object-store`) |
| `crates/object-store/src/lib.rs` | Public exports |
| `crates/object-store/src/object_store.rs` | Public ObjectStore API + internal BlobStore |
| `crates/object-store/src/database.rs` | SQLite pool + migrations |
| `crates/object-store/src/storage.rs` | S3/MinIO/local/memory wrapper + ObjectStoreConfig |
| `crates/object-store/src/actor.rs` | ObjectStoreActor handling proto::Request commands |
| `crates/object-store/src/error.rs` | Error types |

## Files Modified

| File | Changes |
|------|---------|
| `Cargo.toml` | Add workspace member |
| `crates/app/Cargo.toml` | Add object-store dependency |
| `crates/app/src/state.rs` | Add `BlobStoreConfig` enum |
| `crates/app/src/daemon/config.rs` | Replace `node_blobs_store_path` with `blob_store` + `jax_dir` |
| `crates/app/src/daemon/blobs/setup.rs` | Uses BlobsStore constructors for S3/Filesystem configs |
| `crates/app/src/ops/init.rs` | Add blob store CLI flags (`--blob-store`, `--s3-url`, `--blobs-path`) |
| `crates/common/src/peer/blobs_store.rs` | Add `fs()`, `memory()`, `s3()`, `from_store()` constructors |
| `crates/object-store/Cargo.toml` | Add irpc, bao-tree, range-collections dependencies |
| `bin/dev` | Add `minio` and `blob-stores` commands |

## Acceptance Criteria

- [x] `crates/object-store` compiles independently
- [x] Trait bridge implements iroh-blobs store traits (ObjectStoreActor + ObjectStore)
- [x] `jax init --blob-store s3 --s3-url ...` parses correctly
- [x] SQLite metadata can be rebuilt from object storage (`recover_from_storage()`)
- [x] `cargo test` passes
- [x] `cargo clippy` has no warnings

## Future Enhancements

- Partial blob support (track partial state, resume from object storage)
- Full BAO tree traversal for export_bao (currently simplified)

## Verification

```bash
# Start MinIO
./bin/dev minio

# Init with S3 config, then run gateway
jax init --blob-store s3 --s3-url s3://minioadmin:minioadmin@localhost:9000/jax-blobs
jax daemon --gateway
```
