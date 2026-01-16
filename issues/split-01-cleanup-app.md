# Remove Old App Crate

**Status:** Planned
**Epic:** [binary-split.md](./binary-split.md)
**Dependencies:** None (but wait until new binaries are tested in production)

## Objective

Remove the old monolithic `app` crate now that `gateway` and `local` binaries exist.

## Pre-Removal Checklist

Before removing, verify:
- [ ] `jax-gateway` handles all read-only gateway use cases
- [ ] `jax-local` handles all local peer use cases
- [ ] No external dependencies on `jax-bucket` binary name
- [ ] Documentation updated to reference new binaries

## Implementation Steps

1. **Remove app from workspace**
   - Edit `Cargo.toml` workspace members

2. **Delete app crate**
   - Remove `crates/app/` directory

3. **Update any references**
   - Search for `jax-bucket` in docs
   - Update CI/CD if applicable

4. **Clean up**
   - `cargo clean`
   - Verify workspace still builds

## Files to Modify/Delete

- `Cargo.toml` - **MODIFY**: Remove `crates/app` from members
- `crates/app/` - **DELETE**: Entire directory

## Acceptance Criteria

- [ ] `cargo build --workspace` succeeds without app crate
- [ ] `cargo test --workspace` passes
- [ ] No references to old `jax-bucket` binary remain

## Verification

```bash
# After removal
cargo build --workspace
cargo test --workspace

# Ensure old binary doesn't exist
ls target/release/jax-bucket  # Should not exist
ls target/release/jax-gateway # Should exist
ls target/release/jax-local   # Should exist
```

## Rollback Plan

If issues are discovered after removal:
1. Revert the commit removing app crate
2. Document what functionality is missing from new binaries
3. Create tickets to add missing functionality
