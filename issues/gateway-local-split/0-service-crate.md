# Service Crate Extraction

**Status:** Planned

## Objective

Extract shared HTTP server infrastructure into a `crates/service` crate that both gateway and local binaries can use.

## Implementation Steps

1. Create `crates/service/` crate structure
2. Extract gateway handler from `crates/app/src/daemon/http_server/html/gateway/`
3. Define minimal gateway state struct (blobs store, config)
4. Move common HTTP utilities (router setup, middleware)
5. Export types for both consumers

## Files to Modify/Create

### New Files

| File | Description |
|------|-------------|
| `crates/service/Cargo.toml` | New crate manifest |
| `crates/service/src/lib.rs` | Crate entry point |
| `crates/service/src/gateway/mod.rs` | Gateway handler |
| `crates/service/src/gateway/state.rs` | Gateway state |

### Modified Files

| File | Changes |
|------|---------|
| `Cargo.toml` | Add workspace member |
| `crates/app/Cargo.toml` | Add service dependency |
| `crates/app/src/daemon/http_server/` | Remove gateway handler, import from service |

## Acceptance Criteria

- [ ] `crates/service` compiles independently
- [ ] Gateway handler extracted and functional
- [ ] App crate imports gateway from service
- [ ] No code duplication between crates
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings

## Verification

```bash
cargo build -p jax-service
cargo test -p jax-service
cargo clippy -p jax-service
```
