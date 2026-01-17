# jax-common

Core data structures and cryptography for JaxBucket - end-to-end encrypted P2P storage.

## Overview

`jax-common` provides the foundational components for building encrypted, content-addressed storage systems:

- **Cryptography**: ChaCha20-Poly1305 encryption, Ed25519 signatures, X25519 key exchange
- **Mount**: Virtual filesystem abstraction with encrypted file/directory operations
- **Manifest**: Encrypted bucket metadata with access control and version history
- **Peer**: P2P networking via iroh with sync protocol

## Usage

```rust
use common::crypto::{Secret, SecretKey, PublicKey};
use common::mount::{Mount, Manifest, PrincipalRole};
use common::peer::{Peer, PeerBuilder};

// Create encryption key
let secret = Secret::generate();

// Create identity keypair
let secret_key = SecretKey::generate();
let public_key = secret_key.public_key();

// Build a P2P peer
let peer = PeerBuilder::new()
    .secret_key(secret_key)
    .build()
    .await?;

// Create and mount a bucket
let mount = Mount::new(manifest, secret, &peer.blobs_store()).await?;

// File operations
mount.add("/path/to/file", content).await?;
let content = mount.cat("/path/to/file").await?;
let entries = mount.ls("/").await?;
```

## Modules

### `crypto`

Cryptographic primitives:

- `Secret` - ChaCha20-Poly1305 encryption key (256-bit)
- `SecretShare` - X25519 encrypted share of bucket secret
- `SecretKey` / `PublicKey` - Ed25519 identity keypairs

### `mount`

Bucket abstraction:

- `Mount` - In-memory bucket with file operations (add, rm, mv, mkdir, ls, cat)
- `Manifest` - Encrypted bucket metadata (shares, pins, entry point, history)
- `Share` - Principal with optional secret share
- `PrincipalRole` - Owner (full access) or Mirror (read after publish)
- `Node` - File tree nodes with content links

### `peer`

P2P networking:

- `Peer` - iroh-based peer with sync capabilities
- `BlobsStore` - Content-addressed blob storage
- Protocol messages for bucket synchronization

### `linked_data`

Content addressing:

- `Link` - CID wrapper for content references
- IPLD DAG-CBOR serialization

## Features

- **Content-addressed storage**: All data identified by BLAKE3 hash
- **Encryption**: Every file/directory has its own encryption key
- **Access control**: Owner and Mirror roles with cryptographic key sharing
- **Version history**: Immutable manifest chain with previous links
- **P2P sync**: Automatic synchronization via iroh networking

## License

MIT
