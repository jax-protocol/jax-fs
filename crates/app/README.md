# jax-bucket

CLI and daemon for end-to-end encrypted P2P storage buckets.

## Installation

```bash
cargo install jax-bucket
```

## Quick Start

```bash
# Initialize configuration
jax init

# Start the daemon
jax daemon

# Create a bucket
jax bucket create my-bucket

# Add files
jax bucket add <bucket-id> ./file.txt

# List contents
jax bucket ls <bucket-id>
```

## Commands

### Global Options

```bash
jax [OPTIONS] <COMMAND>

Options:
  --remote <URL>       API endpoint (default: http://localhost:3000)
  --config-path <PATH> Config directory (default: ~/.jax)
```

### init

Initialize a new jax configuration directory.

```bash
jax init
```

Creates `~/.jax/` with identity keypair and local database.

### daemon

Start the background service with HTTP API, P2P networking, and web UI.

```bash
jax daemon
```

### daemon --gateway-only

Start a minimal gateway service for content serving only (no UI, no API).

```bash
jax daemon --gateway-only

# Or with full daemon + gateway on separate port
jax daemon --gateway --gateway-url http://localhost:9090
```

The gateway provides:
- P2P peer syncing (mirror role)
- `/gw/:bucket_id/*path` for serving published bucket content with HTML file explorer
- `/_status/livez`, `/_status/readyz`, `/_status/identity` health endpoints
- Content negotiation (`Accept: application/json` for JSON responses)
- `?download=true` query param for raw file downloads

Use `--gateway-only` for lightweight deployments when you only need content serving without the full daemon features.

### version

Display version information.

```bash
jax version
```

## Bucket Commands

### create

```bash
jax bucket create <NAME>
```

### list

```bash
jax bucket list
```

### add

```bash
jax bucket add <BUCKET_ID> <SOURCE_PATH> [DEST_PATH]

# Examples
jax bucket add abc123 ./photo.jpg              # Adds as /photo.jpg
jax bucket add abc123 ./photo.jpg /images/     # Adds as /images/photo.jpg
```

### ls

```bash
jax bucket ls <BUCKET_ID> [PATH]
```

### cat

```bash
jax bucket cat <BUCKET_ID> <PATH>
```

### share

```bash
jax bucket share <BUCKET_ID> --public-key <PEER_PUBLIC_KEY> [--role <ROLE>]

# Roles: owner (full access), mirror (read after publish)
```

### clone

```bash
jax bucket clone <TICKET>
```

### sync

```bash
jax bucket sync <BUCKET_ID>
```

## HTTP API

When the daemon is running, it exposes a REST API at `http://localhost:3000`:

### Health

```
GET /health/live
GET /health/ready
GET /health/version
```

### Bucket API (v0)

```
POST   /api/v0/bucket/create
GET    /api/v0/bucket/list
POST   /api/v0/bucket/:id/add
GET    /api/v0/bucket/:id/ls
GET    /api/v0/bucket/:id/cat
DELETE /api/v0/bucket/:id/delete
POST   /api/v0/bucket/:id/mkdir
POST   /api/v0/bucket/:id/mv
POST   /api/v0/bucket/:id/share
POST   /api/v0/bucket/:id/publish
PUT    /api/v0/bucket/:id/rename
GET    /api/v0/bucket/:id/export
```

## Web UI

The daemon serves a web interface with file explorer, viewer, editor, history, and peer management.

## Gateway

Serves published bucket content over HTTP at `/gw/:bucket_id/*path`.

**Via daemon + gateway:** Run `jax daemon --gateway` to enable gateway on separate port alongside UI and API.

**Via gateway-only:** Run `jax daemon --gateway-only` for standalone gateway (no UI/API).

```bash
# Access content (HTML file explorer)
curl http://localhost:9090/gw/<bucket-id>/

# Access content (JSON)
curl -H "Accept: application/json" http://localhost:9090/gw/<bucket-id>/

# Download raw file
curl "http://localhost:9090/gw/<bucket-id>/file.txt?download=true"
```

Features HTML file explorer, content negotiation, URL rewriting for relative links, and automatic index file serving.

## Configuration

Default location: `~/.jax/`

```
~/.jax/
├── identity.key     # Ed25519 private key
├── database.sqlite  # Local metadata
└── blobs/           # Content-addressed storage
```

## Environment Variables

```bash
RUST_LOG=debug       # Debug logging
RUST_BACKTRACE=1     # Backtraces on panic
```

## License

MIT
