# Gateway and Tauri Split

**Status:** Complete

## Summary

Restructured the daemon architecture: separated API (private, port 5001) and gateway (public, port 8080) onto separate ports, removed the Askama web UI, added SQLite + S3 blob storage, pluggable conflict resolution, and built a Tauri 2.0 desktop app with SolidJS replacing the old HTML UI.

## Architecture (Final)

```
jax daemon (headless service)
├── P2P peer (owner/mirror roles)
├── API server on port 5001 (private, mutation/RPC)
│   ├── REST API at /api/v0/*
│   └── Health checks at /_status/*
├── Gateway server on port 8080 (public, read-only)
│   ├── Content serving at /gw/*
│   └── Health checks at /_status/*

jax-desktop (Tauri app - separate binary)
├── SolidJS frontend (Vite)
├── Tauri IPC → daemon REST API via HTTP
├── System tray + auto-launch
└── Full bucket management UI
```

## Tickets

| # | Ticket | Status |
|---|--------|--------|
| 0 | Gateway subcommand | Done |
| 1 | SQLite blob store | Done |
| 2 | Conflict resolution | Done |
| 3 | Daemon simplification | Done |
| 4 | Tauri desktop app | Done |

FUSE integration (formerly ticket 5) moved to its own epic: [`issues/fuse-integration/`](../fuse-integration/index.md)

## Reference Branches

| Branch | Reference For |
|--------|---------------|
| `amiller68/sqlite-minio-blobs` | SQLite + Object Storage blob backend |
| `amiller68/conflict-resolution` | Conflict resolution strategies |
| `amiller68/tauri-app-explore` | Tauri + SolidJS prototype |
