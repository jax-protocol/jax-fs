# ImportBao Size Rejections During Sync

**Status:** Planned

## Objective

Investigate and fix the spurious ImportBao size rejections that occur during P2P blob sync, where blob sizes appear corrupted/garbage during negotiation.

## Background

During e2e testing, we observe errors like:

```
ERROR jax_blobs_store::actor: ImportBao: rejecting import of hash fa81500eb86ed618a55390a67dc2d987277a008c283db1e3e8ca278cab15cdea with unreasonable size 8988579870866186464 (max is 1073741824)
```

The reported sizes (e.g., `8988579870866186464`) are clearly garbage values - they exceed petabytes. Despite these rejections, blobs eventually sync correctly through retries.

## Observations

- Occurs during initial P2P discovery/sync between nodes
- Reported sizes are impossibly large (corrupted u64 values)
- Blobs do eventually sync correctly (transient issue)
- May be related to BAO protocol handshake or stream framing
- The max size check (1GB) was added in commit `25495a2` to prevent OOM crashes

## Possible Causes

1. **BAO stream framing issue**: Size bytes being read before stream is properly aligned
2. **Concurrent stream corruption**: Multiple sync requests interfering
3. **iroh-blobs protocol mismatch**: Version or encoding differences
4. **Endianness issue**: Size being read with wrong byte order

## Files to Investigate

| File | Area |
|------|------|
| `crates/object-store/src/actor.rs` | ImportBao handler (ObjectStoreActor) |
| `crates/common/src/peer/sync/` | Sync job implementation |

## Acceptance Criteria

- [ ] Root cause identified
- [ ] Fix implemented (or documented as upstream issue)
- [ ] No spurious ImportBao rejections during normal sync
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings

## Verification

```bash
./bin/dev kill --force && ./bin/dev clean
./bin/dev run --background
sleep 90
./bin/dev logs grep "ImportBao: rejecting" | wc -l
# Should be 0 after fix
```
