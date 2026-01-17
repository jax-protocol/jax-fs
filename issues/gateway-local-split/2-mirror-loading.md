# Mirror Loading

**Status:** Planned

## Objective

Enable gateway to load published buckets as a mirror principal, decrypting content using the published secret from the manifest.

## Implementation Steps

1. Add bucket loading from manifest URL
2. Use `published_secret` from manifest for decryption
3. Implement read-only mount operations
4. Return appropriate errors for unpublished buckets
5. Handle multiple bucket sources

## Files to Modify/Create

### Modified Files

| File | Changes |
|------|---------|
| `crates/gateway/src/state.rs` | Add bucket loading logic |
| `crates/service/src/gateway/mod.rs` | Pass bucket context to handlers |

## Acceptance Criteria

- [ ] Gateway loads buckets from manifest URLs
- [ ] Decryption works with published_secret
- [ ] Unpublished buckets return 403 Forbidden
- [ ] Multiple buckets can be served simultaneously
- [ ] Content-addressed blob fetching works
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings

## Verification

```bash
# Publish a bucket via local client
jax-bucket bucket publish my-bucket

# Serve via gateway
jax-gateway --bucket /path/to/manifest

# Verify content is accessible
curl http://localhost:8080/
```
