# Gateway Subcommand

**Status:** Done
**Track:** Gateway

## Objective

Add gateway-only mode to the daemon that runs a minimal gateway service: P2P peer (mirror role) + gateway content serving.

## Implementation

Instead of a separate `jax gw` subcommand, the gateway is integrated into the daemon command:

- `jax daemon --gateway-only` - Run only the gateway server (no HTML UI, no API)
- `jax daemon --gateway --gateway-url <url>` - Run full daemon + gateway on separate port

This approach:
- Reuses the existing daemon infrastructure
- Avoids code duplication
- Provides consistent configuration via `config.toml`

## Files Created

| File | Description |
|------|-------------|
| `crates/app/templates/pages/gateway/index.html` | Gateway identity/root page |
| `crates/app/templates/pages/gateway/explorer.html` | Read-only directory browser |
| `crates/app/templates/pages/gateway/viewer.html` | Read-only file viewer |

## Files Modified

| File | Changes |
|------|---------|
| `crates/app/src/ops/daemon.rs` | Add `--gateway-only`, `--gateway`, `--gateway-url` flags |
| `crates/app/src/daemon/process/mod.rs` | Add `spawn_gateway_service` function |
| `crates/app/src/daemon/http_server/html/gateway/mod.rs` | Add HTML templates and content negotiation |

## Acceptance Criteria

- [x] `jax daemon --gateway-only` starts gateway server
- [x] P2P peer syncs as mirror role
- [x] `/gw/:bucket_id/*` serves published content with HTML UI
- [x] `Accept: application/json` header returns JSON for programmatic access
- [x] `?download=true` returns raw file download
- [x] No Askama UI routes available in gateway-only mode
- [x] No REST API routes available in gateway-only mode
- [x] `cargo test` passes
- [x] `cargo clippy` has no warnings

## Verification

```bash
# Start gateway-only
cargo run -- --config-path ./data/node3 daemon --gateway-only

# In another terminal, start full daemon
cargo run -- --config-path ./data/node1 daemon

# Verify gateway serves content (HTML explorer)
open http://localhost:9092/gw/<bucket-id>

# Verify JSON API
curl -H "Accept: application/json" http://localhost:9092/gw/<bucket-id>

# Verify raw download
curl "http://localhost:9092/gw/<bucket-id>/file.txt?download=true"

# Verify UI is NOT available
curl http://localhost:9092/buckets  # Should 404

# Verify identity endpoint
curl http://localhost:9092/_status/identity
```
