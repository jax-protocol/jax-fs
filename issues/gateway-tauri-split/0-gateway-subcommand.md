# Gateway Subcommand

**Status:** Done
**Track:** Gateway
**Branch:** `alex/gateway-subcommand`

## Completion Notes

We did not make this a subcommand, and instead implemented this as a new flag.

## Objective

Add gateway-only mode to the daemon that runs a minimal gateway service: P2P peer (mirror role) + gateway content serving.

## Implementation

Instead of a separate `jax gw` subcommand, the gateway is integrated into the daemon command:

- `jax daemon --gateway` - Run only the gateway server (no App UI, no API)
- `jax daemon --with-gateway` - Run full daemon + gateway on separate ports
- `jax daemon --gateway-port <port>` - Override gateway port (implies `--with-gateway`)

This approach:
- Reuses the existing daemon infrastructure (single `spawn_service` function)
- Avoids code duplication
- Provides consistent configuration via `config.toml`

### CLI Flags

| Flag | Description |
|------|-------------|
| `--gateway` | Gateway-only mode (no App server) |
| `--with-gateway` | Run App + Gateway together |
| `--gateway-port <port>` | Override gateway port |
| `--gateway-url <url>` | External gateway URL for share links |
| `--api-url <url>` | API URL for HTML UI |

### Architecture

The server spawning was consolidated into a single `spawn_service` function that conditionally starts:
- App server (Askama UI + REST API) on `app_port`
- Gateway server (read-only content serving) on `gateway_port`

Both servers share the same `ServiceState` (peer, database, etc.).

## Files Created

| File | Description |
|------|-------------|
| `crates/app/templates/pages/gateway/index.html` | Gateway identity/root page |
| `crates/app/templates/pages/gateway/explorer.html` | Read-only directory browser |
| `crates/app/templates/pages/gateway/viewer.html` | Read-only file viewer |

## Files Modified

| File | Changes |
|------|---------|
| `crates/app/src/ops/daemon.rs` | Add `--gateway`, `--with-gateway`, `--gateway-port`, `--gateway-url` flags |
| `crates/app/src/daemon/mod.rs` | Export `ServiceConfig`, `spawn_service` |
| `crates/app/src/daemon/config.rs` | Add `api_url`, `gateway_url` fields |
| `crates/app/src/daemon/process.rs` | Consolidated server spawning logic |
| `crates/app/src/daemon/http_server/html/gateway/mod.rs` | Gateway handlers with content negotiation |

## Acceptance Criteria

- [x] `jax daemon --gateway` starts gateway-only server
- [x] P2P peer syncs as mirror role
- [x] `/gw/:bucket_id/*` serves published content with HTML UI
- [x] `Accept: application/json` header returns JSON for programmatic access
- [x] `?download=true` returns raw file download
- [x] `?deep=true` returns recursive directory listing
- [x] No Askama UI routes available in gateway-only mode
- [x] No REST API routes available in gateway-only mode
- [x] `cargo test` passes
- [x] `cargo clippy` has no warnings

## Verification

```bash
# Start gateway-only
cargo run -- --config-path ./data/node3 daemon --gateway

# In another terminal, start full daemon
cargo run -- --config-path ./data/node1 daemon

# Verify gateway serves content (HTML explorer)
open http://localhost:9092/gw/<bucket-id>

# Verify JSON API
curl -H "Accept: application/json" http://localhost:9092/gw/<bucket-id>

# Verify deep listing
curl -H "Accept: application/json" "http://localhost:9092/gw/<bucket-id>?deep=true"

# Verify raw download
curl "http://localhost:9092/gw/<bucket-id>/file.txt?download=true"

# Verify UI is NOT available
curl http://localhost:9092/buckets  # Should 404

# Verify identity endpoint
curl http://localhost:9092/_status/identity
```

## Downstream Impact

**Ticket 1 (SQLite blob store):** CLI examples updated to use `--gateway` instead of `--gateway-only`
