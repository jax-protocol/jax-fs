# Gateway and Local Binary Split

## Background

The current `jax-bucket` binary combines two distinct responsibilities:

1. **Gateway functionality**: Serving bucket content over HTTP (the gateway handler)
2. **Local client functionality**: Full daemon with syncing, file operations, sharing, and publishing

This coupling limits deployment flexibility. We want to run lightweight, stateless gateway instances on edge infrastructure to serve published buckets, while keeping the full-featured local client for bucket owners and editors.

## Goals

- **Gateway binary (`jax-gateway`)**: Minimal, read-only HTTP server for serving published bucket content
- **Local binary (`jax-local` or keep `jax-bucket`)**: Full client with daemon, sync, editing, sharing, publishing
- **Shared code**: Common crate already contains crypto, mount, and peer protocol

## Tickets

| # | Ticket | Status |
|---|--------|--------|
| 0 | [Service crate extraction](./0-service-crate.md) | Planned |
| 1 | [Gateway binary](./1-gateway-binary.md) | Planned |
| 2 | [Mirror loading](./2-mirror-loading.md) | Planned |
| 3 | [Local refactor](./3-local-refactor.md) | Planned |
| 4 | [Deployment & docs](./4-deployment.md) | Planned |

## Architecture Decisions

### Gateway

```
jax-gateway
├── Reads published bucket manifests (mirrors have decryption access)
├── Serves content via HTTP gateway handler
├── Stateless - no persistent state needed
├── No secret key management (uses mirror shares from published buckets)
└── Can be horizontally scaled on CDN/edge
```

### Local

```
jax-local (or jax-bucket)
├── Full daemon with peer-to-peer syncing
├── Mount operations (add, rm, mv, mkdir, ls, cat)
├── Bucket management (create, share, publish/unpublish)
├── Secret key management
└── Interactive CLI
```

## Key Technical Decisions

1. **Mirror principal role** (COMPLETE): Mirrors can sync bucket data but only decrypt when published
2. **Publish workflow** (COMPLETE): Grants mirrors decryption access via plaintext secret in manifest
3. **Gateway doesn't need full peer**: Just needs blob storage and manifest reading
4. **URL rewriting**: Gateway already has URL rewriting and index file support

## Phase 1: Foundation (COMPLETE)

- [x] Add `PrincipalRole::Mirror` to principal system
- [x] Implement `Option<SecretShare>` for mirrors (None until published)
- [x] Add publish workflow to manifest and mount
- [x] Extend `/share` endpoint with role parameter
- [x] Add `/publish` endpoint
- [x] Integration tests for mirror mounting

## Verification Checklist

- [ ] Gateway can serve published bucket content without owner keys
- [ ] Gateway returns appropriate errors for unpublished buckets
- [ ] Local client retains all existing functionality
- [ ] Gateway is stateless and horizontally scalable
- [ ] Integration tests pass for both binaries

## Related Files

- `crates/common/src/mount/principal.rs` - PrincipalRole enum
- `crates/common/src/mount/manifest.rs` - Share with Option<SecretShare>
- `crates/app/src/daemon/http_server/html/gateway/` - Gateway handler (to be extracted)
