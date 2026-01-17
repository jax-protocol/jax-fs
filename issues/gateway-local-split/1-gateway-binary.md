# Gateway Binary

**Status:** Planned

## Objective

Create `jax-gateway` binary crate that serves published bucket content without requiring full daemon functionality.

## Implementation Steps

1. Create `crates/gateway/` binary crate
2. Implement gateway-specific CLI:
   - `--port` - HTTP port
   - `--config` - Config file path
   - `--bucket` - Bucket sources (manifest URLs or local paths)
3. Import gateway handler from service crate
4. Implement minimal state initialization (blob store, no peer)
5. No daemon, no P2P syncing - just static serving

## Files to Modify/Create

### New Files

| File | Description |
|------|-------------|
| `crates/gateway/Cargo.toml` | Binary crate manifest |
| `crates/gateway/src/main.rs` | Entry point |
| `crates/gateway/src/args.rs` | CLI argument parsing |
| `crates/gateway/src/config.rs` | Gateway configuration |
| `crates/gateway/src/state.rs` | Minimal gateway state |

### Modified Files

| File | Changes |
|------|---------|
| `Cargo.toml` | Add workspace member |

## Acceptance Criteria

- [ ] `jax-gateway` binary compiles
- [ ] CLI accepts port, config, bucket arguments
- [ ] Gateway starts and serves HTTP
- [ ] No P2P peer required
- [ ] Stateless operation (no database)
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings

## Verification

```bash
cargo build -p jax-gateway
cargo run -p jax-gateway -- --help
cargo run -p jax-gateway -- --port 8080
```
