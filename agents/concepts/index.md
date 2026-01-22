# Concepts

This directory contains detailed documentation about JaxBucket's architecture and design.

## Documents

| Document | Description |
|----------|-------------|
| [Overview](./overview.md) | High-level architecture, layers, and core concepts |
| [Data Model](./data-model.md) | Buckets, Manifests, Nodes, Pins, and Bucket Log |
| [Cryptography](./cryptography.md) | Identity, key sharing, and content encryption |
| [Synchronization](./synchronization.md) | Peer structure and sync protocol |
| [Conflict Resolution](./conflict-resolution.md) | Pluggable conflict handling for concurrent edits |
| [Security](./security.md) | Threat model, best practices, and implementation details |

## Quick Overview

JaxBucket is a peer-to-peer, encrypted storage system that combines:

1. **Content Addressing**: Files and directories stored as BLAKE3-hashed blobs
2. **Encryption**: Each file/directory has its own encryption key
3. **P2P Networking**: Built on Iroh's QUIC-based networking stack
4. **Merkle DAGs**: Immutable, hash-linked data structures

```text
┌─────────────────────────────────────────────────┐
│                   JaxBucket                      │
├─────────────────────────────────────────────────┤
│  ┌──────────┐  ┌──────────┐  ┌──────────────┐  │
│  │ Buckets  │  │  Crypto  │  │ Sync Manager │  │
│  │  (DAG)   │  │(ECDH+AES)│  │(Pull/Push)   │  │
│  └────┬─────┘  └─────┬────┘  └──────┬───────┘  │
├───────┼──────────────┼───────────────┼──────────┤
│  ┌────▼──────────────▼───────────────▼───────┐  │
│  │        Iroh Networking Layer              │  │
│  │  (QUIC + DHT Discovery + BlobStore)       │  │
│  └───────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
```

## Reading Order

For a complete understanding, read in this order:

1. **[Overview](./overview.md)** - Start here for the big picture
2. **[Data Model](./data-model.md)** - Understand how data is structured
3. **[Cryptography](./cryptography.md)** - Learn how encryption works
4. **[Synchronization](./synchronization.md)** - See how peers sync data
5. **[Conflict Resolution](./conflict-resolution.md)** - Learn how concurrent edits are handled
6. **[Security](./security.md)** - Understand security guarantees and limitations
