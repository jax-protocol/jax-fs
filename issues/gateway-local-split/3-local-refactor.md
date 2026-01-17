# Local Binary Refactor

**Status:** Planned

## Objective

Clean separation of local-only code in `jax-bucket`, ensuring it retains all current functionality while gateway-specific code lives in the service crate.

## Implementation Steps

1. Audit current app crate for gateway dependencies
2. Ensure clean imports from service crate
3. Verify all local functionality preserved:
   - Full daemon with P2P peer
   - Mount operations (add, rm, mv, mkdir, ls, cat)
   - Bucket management (create, share, publish)
   - Secret key management
   - Interactive CLI
4. Remove any dead code from gateway extraction
5. Update internal documentation

## Files to Modify/Create

### Modified Files

| File | Changes |
|------|---------|
| `crates/app/src/daemon/http_server/` | Clean imports from service |
| `crates/app/src/daemon/mod.rs` | Remove gateway-specific code |

## Acceptance Criteria

- [ ] All existing CLI commands work unchanged
- [ ] All existing API endpoints work unchanged
- [ ] P2P sync functionality preserved
- [ ] No dead code remaining
- [ ] `cargo test -p jax-bucket` passes
- [ ] `cargo clippy` has no warnings

## Verification

```bash
# Run full test suite
cargo test -p jax-bucket

# Verify CLI commands
jax-bucket bucket create test
jax-bucket bucket add test ./file.txt
jax-bucket bucket ls test
jax-bucket bucket share test <pubkey>
jax-bucket bucket publish test

# Verify daemon
jax-bucket daemon
```
