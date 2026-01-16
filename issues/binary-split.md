# Split Gateway and Local Binaries

**Status:** In Progress

## Background

The original `jax-bucket` application was a single binary combining:
- CLI commands for bucket management
- HTTP server with HTML UI
- JSON API for write operations
- Gateway for serving bucket content

This monolithic approach doesn't fit deployment scenarios where:
- A **gateway** runs on a server, serving public bucket content (read-only)
- A **local peer** runs on a user's machine with full read/write access
- The CLI is used interactively (may not be needed as a separate tool)

## Target Architecture

```
crates/
├── common/           # Core: peer, mount, crypto, protocol
├── service/          # Shared: database, sync, HTTP handlers, templates
├── gateway/          # Binary: jax-gateway (read-only, serves published buckets)
├── local/            # Binary: jax-local (full-featured, write API)
└── app/              # OLD: To be removed after migration
```

### Dependency Graph

```
              common
                 │
              service
                 │
        ┌───────┴───────┐
        │               │
     gateway          local
```

## Current State (What's Done)

### Service Crate (`crates/service/`)

Shared infrastructure extracted from `app`:

```
service/
├── src/
│   ├── lib.rs
│   ├── config.rs           # Service configuration
│   ├── state.rs            # ServiceState (peer + database)
│   ├── sync_provider.rs    # Background job queue
│   ├── database/           # SQLite BucketLogProvider
│   └── http/
│       ├── config.rs       # HTTP server config
│       ├── handlers/       # Shared handlers
│       ├── health/         # Health check endpoints
│       └── html/           # HTML UI + gateway handler
├── templates/              # Askama templates
├── static/                 # CSS, JS, fonts
└── migrations/             # SQLite migrations
```

### Gateway Binary (`crates/gateway/`)

Read-only gateway for serving published buckets:

```bash
jax-gateway [OPTIONS]
  -p, --port <PORT>           # HTTP port [default: 8080]
  -d, --database <DATABASE>   # SQLite database path
  -b, --blobs <BLOBS>         # Blobs storage path
      --peer-port <PORT>      # P2P networking port
      --log-level <LEVEL>     # Log level [default: info]
```

Features:
- Gateway route: `GET /gw/:bucket_id/*path`
- Health endpoints: `/_status/livez`, `/_status/readyz`
- Markdown rendering, URL rewriting, index file detection
- Single port, no write API

### Local Binary (`crates/local/`)

Full-featured local peer with write API:

```bash
jax-local [OPTIONS]
      --html-port <PORT>      # HTML UI port [default: 8080]
      --api-port <PORT>       # API port [default: 3000]
  -d, --database <DATABASE>   # SQLite database path
  -b, --blobs <BLOBS>         # Blobs storage path
      --peer-port <PORT>      # P2P networking port
      --log-level <LEVEL>     # Log level [default: info]
      --api-hostname <HOST>   # API hostname for UI
      --read-only             # Disable write operations
```

Features:
- Full HTML UI with bucket management
- JSON API for CRUD operations
- Gateway route for local preview
- Health endpoints
- Two ports: HTML UI and API

## What's Missing

1. **Remove old app crate** - Clean up the monolithic app
2. **Gateway access control** - Only serve published buckets (depends on public-buckets epic)
3. **Mirror mode for gateway** - Gateway runs as mirror peer, not owner

## Tickets

| Ticket | Description | Status | Dependencies |
|--------|-------------|--------|--------------|
| [split-01-cleanup-app](./split-01-cleanup-app.md) | Remove old app crate | Planned | None |
| [split-02-mirror-mode](./split-02-mirror-mode.md) | Gateway runs as mirror peer | Planned | public-buckets |

## Key Decisions

### Why Separate Binaries?

| Aspect | Gateway | Local |
|--------|---------|-------|
| Deployment | Server | Desktop |
| Access | Read-only | Read/write |
| Peer type | Mirror | Owner |
| Database | Own SQLite | Own SQLite |
| HTTP ports | Single (8080) | Dual (8080, 3000) |
| API | None | Full JSON API |
| Can decrypt | Published only | All owned |

### Why Keep Templates in Service?

Both binaries need the same templates:
- Gateway serves the file explorer for published buckets
- Local serves the full bucket management UI

Keeping templates in `service/` means:
- Single source of truth for UI
- Both binaries stay in sync
- Easier maintenance

### CLI Removed

The original app had CLI commands (`jax bucket create`, etc.). These are removed because:
- The local binary provides full API access
- Users can use `curl` or a future client library
- Simplifies the codebase

## Verification Checklist

- [x] `cargo build -p jax-gateway` succeeds
- [x] `cargo build -p jax-local` succeeds
- [x] Gateway starts and serves health endpoints
- [x] Local starts with HTML UI and API
- [x] Both binaries share templates correctly
- [ ] Old app crate removed
- [ ] Gateway runs in mirror mode
- [ ] Gateway only serves published buckets
