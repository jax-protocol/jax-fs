# Gateway Subcommand

**Status:** Planned
**Track:** Gateway

## Objective

Add `jax gw` subcommand that runs a minimal gateway service: P2P peer (mirror role) + gateway content serving.

## Implementation Steps

1. Add `Gw` variant to CLI command enum in `crates/app/src/main.rs`
2. Create `crates/app/src/ops/gw.rs` for gateway command handling
3. Extract gateway server setup from daemon (reuse existing gateway handler)
4. Initialize P2P peer with mirror role only
5. Remove Askama UI routes, remove REST API routes - keep only `/gw/:bucket_id/*`

## Files to Create

| File | Description |
|------|-------------|
| `crates/app/src/ops/gw.rs` | Gateway subcommand implementation |

## Files to Modify

| File | Changes |
|------|---------|
| `crates/app/src/main.rs` | Add `Gw` to CLI enum |
| `crates/app/src/ops/mod.rs` | Add gw module |

## Acceptance Criteria

- [ ] `jax gw --port 8080` starts gateway server
- [ ] P2P peer syncs as mirror role
- [ ] `/gw/:bucket_id/*` serves published content
- [ ] No Askama UI routes available
- [ ] No REST API routes available
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings

## Verification

```bash
# Start gateway
cargo run -- gw --port 8080

# In another terminal, publish a bucket via daemon
cargo run -- daemon &
# ... publish a bucket ...

# Verify gateway serves content
curl http://localhost:8080/gw/<bucket-id>/index.html

# Verify UI is NOT available
curl http://localhost:8080/buckets  # Should 404
```
