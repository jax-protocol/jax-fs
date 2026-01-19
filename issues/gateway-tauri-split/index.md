# Gateway and Local Mode Split

## Background

The current `jax daemon` combines full local client functionality (Askama UI, REST API, P2P sync) with gateway serving. This limits deployment flexibility for edge/CDN use cases.

We added a `--gateway-only` mode to the daemon that runs a minimal gateway service: P2P peer (mirror role) + gateway content serving + SQLite/Object Storage backend.

## Architecture

```
jax daemon (full local client)
├── P2P peer (owner/mirror roles)
├── Askama web UI
├── REST API
└── Gateway handler (optional, via --gateway flag)

jax daemon --gateway-only (gateway mode)
├── P2P peer (mirror role)
├── Gateway handler with read-only HTML file explorer
└── SQLite + Object Storage blob backend
```

## Tickets

| # | Ticket | Status | Track |
|---|--------|--------|-------|
| 0 | [Gateway subcommand](./0-gateway-subcommand.md) | Done | Gateway |
| 1 | [SQLite blob store](./1-sqlite-blobstore.md) | Planned | Gateway |
| 2 | [Conflict resolution](./2-conflict-resolution.md) | Planned | Common |
| 3 | [FUSE integration](./3-fuse-integration.md) | Planned | Local |
| 4 | [Desktop integration](./4-desktop-integration.md) | Planned | Local |
| 5 | [Tauri migration](./5-tauri-migration.md) | Future | Local |

## Execution Order

**Stage 1 (Foundation):**
- Ticket 0: Gateway subcommand (`jax daemon --gateway-only`) - **Done**

**Stage 2 (Parallel Tracks):**

| Gateway Track | Common/Local Track |
|---------------|-------------------|
| Ticket 1: SQLite blob store | Ticket 2: Conflict resolution |
| | Ticket 3: FUSE integration |
| | Ticket 4: Desktop integration |

## Reference Branches

| Branch | Reference For |
|--------|---------------|
| `amiller68/sqlite-minio-blobs` | SQLite + Object Storage blob backend |
| `amiller68/fs-over-blobstore-v1` | FUSE implementation |
| `amiller68/conflict-resolution` | Conflict resolution strategies |
