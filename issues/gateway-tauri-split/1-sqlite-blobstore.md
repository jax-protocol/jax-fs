# Gateway SQLite + Object Storage Blob Store

**Status:** Planned
**Track:** Gateway
**Reference:** `amiller68/sqlite-minio-blobs` branch, `issues/sqlite-object-storage-blobs.md`

## Objective

Integrate SQLite + Object Storage blob backend into gateway for cloud-native deployments.

## Implementation Steps

1. Create `crates/blobs-store/` crate (from reference branch)
2. Implement iroh-blobs store trait bridge
3. Add blob store config to gateway command (S3 endpoint, bucket, credentials)
4. Wire blob store into gateway's peer

## Files to Create

| File | Description |
|------|-------------|
| `crates/blobs-store/Cargo.toml` | New crate |
| `crates/blobs-store/src/lib.rs` | Public exports |
| `crates/blobs-store/src/store.rs` | Main BlobStore API |
| `crates/blobs-store/src/database.rs` | SQLite pool + migrations |
| `crates/blobs-store/src/object_store.rs` | S3/MinIO wrapper |

## Files to Modify

| File | Changes |
|------|---------|
| `Cargo.toml` | Add workspace member |
| `crates/app/Cargo.toml` | Add blobs-store dependency |
| `crates/app/src/ops/daemon.rs` | Add blob store config to gateway flags |

## Acceptance Criteria

- [ ] `crates/blobs-store` compiles independently
- [ ] Trait bridge implements iroh-blobs store traits
- [ ] `jax daemon --gateway-only --blob-store s3 --s3-endpoint ...` works
- [ ] SQLite metadata can be rebuilt from object storage
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings

## Verification

```bash
# Start MinIO
./bin/minio.sh start

# Start gateway with S3 config
cargo run -- daemon --gateway-only --blob-store s3 --s3-endpoint http://localhost:9000 --s3-bucket jax-blobs

# Verify blobs stored in MinIO
mc ls minio/jax-blobs
```
