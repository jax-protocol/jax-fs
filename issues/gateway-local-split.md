# Gateway and Local Binary Split

**Status:** Planned

## Background

The current `jax-bucket` binary combines two distinct responsibilities:

1. **Gateway functionality**: Serving bucket content over HTTP (the gateway handler)
2. **Local client functionality**: Full daemon with syncing, file operations, sharing, and publishing

This coupling limits deployment flexibility. We want to run lightweight, stateless gateway instances on edge infrastructure to serve published buckets, while keeping the full-featured local client for bucket owners and editors.

## Goals

- **Gateway binary (`jax-gateway`)**: Minimal, read-only HTTP server for serving published bucket content
- **Local binary (`jax-local` or keep `jax-bucket`)**: Full client with daemon, sync, editing, sharing, publishing
- **Shared code**: Common crate already contains crypto, mount, and peer protocol

## Architecture

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
2. **Publish workflow** (COMPLETE): Grants mirrors decryption access via secret share encryption
3. **Gateway doesn't need full peer**: Just needs blob storage and manifest reading
4. **URL rewriting**: Gateway already has URL rewriting and index file support

## Phases

### Phase 1: Foundation (COMPLETE)
- [x] Add `PrincipalRole::Mirror` to principal system
- [x] Implement `Option<SecretShare>` for mirrors (None until published)
- [x] Add publish/unpublish workflow to manifest and mount
- [x] Extend `/share` endpoint with role parameter
- [x] Add `/publish` endpoint
- [x] Integration tests for mirror mounting

### Phase 2: Service Crate Extraction
- [ ] Create `crates/service` with shared HTTP server infrastructure
- [ ] Extract gateway handler from app crate
- [ ] Define minimal gateway state (blobs store, config)

### Phase 3: Gateway Binary
- [ ] Create `crates/gateway` binary crate
- [ ] Implement gateway-specific CLI (config, port, bucket sources)
- [ ] Gateway loads buckets as mirror (read-only)
- [ ] No daemon, no peer syncing - just static serving

### Phase 4: Local Binary Refactor
- [ ] Rename or keep `jax-bucket` as local client
- [ ] Ensure clean separation from gateway code
- [ ] Local retains all current functionality

### Phase 5: Deployment & Documentation
- [ ] Docker images for gateway
- [ ] Deployment guide for edge/CDN setup
- [ ] Update CLI documentation

## Child Tickets

- `gateway-01-service-crate.md` - Extract shared HTTP infrastructure
- `gateway-02-binary-setup.md` - Create gateway binary and CLI
- `gateway-03-mirror-loading.md` - Gateway loads published buckets as mirror
- `local-01-refactor.md` - Clean separation of local-only code

## Verification Checklist

- [ ] Gateway can serve published bucket content without owner keys
- [ ] Gateway returns appropriate errors for unpublished buckets
- [ ] Local client retains all existing functionality
- [ ] Gateway is stateless and horizontally scalable
- [ ] Integration tests pass for both binaries

## Related

- PR: Mirror principal role and publishing workflow (current)
- `crates/common/src/mount/principal.rs` - PrincipalRole enum
- `crates/common/src/mount/manifest.rs` - Share with Option<SecretShare>
- `crates/app/src/daemon/http_server/gateway/` - Gateway handler (to be extracted)
