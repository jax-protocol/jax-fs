# Storage Guide

This guide covers data persistence in jax-bucket. There is no traditional database - all data is stored as content-addressed blobs.

## Overview

- **Storage**: Content-addressed blobs via iroh-blobs
- **Format**: IPLD DAG-CBOR for structured data
- **Encryption**: ChaCha20-Poly1305 for all bucket content
- **Location**: `~/.jax/blobs/` (configurable)

---

## Content-Addressed Storage

All data is stored as blobs identified by their BLAKE3 hash:

```rust
use common::peer::BlobsStore;

// Store a blob
let hash = blobs.put(data).await?;

// Retrieve a blob
let data = blobs.get(&hash).await?;
```

### Why Content-Addressed?

- **Deduplication**: Same content = same hash = stored once
- **Integrity**: Hash verifies content hasn't been tampered with
- **P2P friendly**: Peers can verify data without trusting source
- **Immutable**: Content never changes, only new versions created

---

## BlobsStore

The `BlobsStore` wraps iroh-blobs for local blob storage:

```rust
use common::peer::BlobsStore;

// Create file-system backed store
let blobs = BlobsStore::fs(&path).await?;

// Store data
let hash = blobs.put(data).await?;

// Store streaming data
let hash = blobs.put_stream(reader).await?;

// Retrieve data
let data: Vec<u8> = blobs.get(&hash).await?;

// Check if blob exists
let exists = blobs.has(&hash).await?;
```

---

## Data Model

### Manifest

The manifest is the root of a bucket's state:

```rust
pub struct Manifest {
    shares: BTreeMap<String, Share>,  // Public key -> encrypted share
    pins: Link,                        // Pinned blob hashes
    entry: Link,                       // Root of file tree
    previous: Option<Link>,            // Previous manifest (history)
    ops_log: Option<Link>,             // Operation log for CRDT
}
```

### Share

Each principal has a share with access information:

```rust
pub struct Share {
    principal: Principal,
    share: Option<SecretShare>,  // None for unpublished mirrors
}

pub struct Principal {
    role: PrincipalRole,         // Owner or Mirror
    identity: PublicKey,
}
```

### Node

File tree nodes link to content:

```rust
pub enum NodeLink {
    Dir(Link),           // Link to child directory
    Data(Link, DataInfo), // Link to file content with metadata
}

pub struct DataInfo {
    size: u64,
    mime: Option<String>,
}
```

---

## Encryption

All bucket content is encrypted with AES-GCM:

```rust
use common::crypto::Secret;

// Generate a new secret
let secret = Secret::generate();

// Encrypt data
let ciphertext = secret.encrypt(&plaintext)?;

// Decrypt data
let plaintext = secret.decrypt(&ciphertext)?;
```

### Secret Sharing

The bucket secret is shared with principals via X25519:

```rust
use common::crypto::SecretShare;

// Create share for a public key
let share = SecretShare::new(&secret, &public_key)?;

// Recover secret with corresponding secret key
let secret = share.recover(&secret_key)?;
```

---

## Mount Operations

The `Mount` struct provides high-level bucket operations:

```rust
use common::mount::Mount;

// Create new bucket
let mount = Mount::init(id, name, &secret_key, &blobs).await?;

// Load existing bucket
let mount = Mount::load(&link, &secret_key, &blobs).await?;

// Save bucket (returns new manifest link)
let (link, previous, height) = mount.save(&blobs).await?;

// File operations
mount.add(&path, reader).await?;
mount.cat(&path).await?;
mount.ls(&path).await?;
mount.rm(&path).await?;
mount.mkdir(&path).await?;
mount.mv(&from, &to).await?;
```

---

## Persisting State

### Save Flow

1. Encrypt file tree nodes
2. Store encrypted blobs
3. Update manifest with new entry link
4. Encrypt and store manifest
5. Return manifest hash as bucket's new head

```rust
// Save returns the new manifest link
let (link, previous_link, height) = mount.save(&blobs).await?;

// The link can be shared with peers to sync
```

### Load Flow

1. Fetch manifest blob by hash
2. Decrypt manifest with user's share
3. Fetch file tree from entry link
4. Decrypt nodes as needed

```rust
// Load from manifest link
let mount = Mount::load(&link, &secret_key, &blobs).await?;

// Mirrors can only load if published
if mount_result.is_err() && matches!(mount_result, Err(MountError::MirrorCannotMount)) {
    // Bucket not published yet
}
```

---

## P2P Sync

Buckets sync between peers by exchanging blob hashes:

1. **Announce**: Peer shares its manifest hash
2. **Compare**: Peers identify missing blobs
3. **Transfer**: Missing blobs are fetched
4. **Merge**: Operation logs merged via CRDT

The `BlobsStore` handles blob transfer via iroh's P2P protocol.

---

## Testing with Storage

Use `TempDir` for isolated test storage:

```rust
use tempfile::TempDir;

#[tokio::test]
async fn test_storage() {
    let temp_dir = TempDir::new().unwrap();
    let blobs = BlobsStore::fs(&temp_dir.path().join("blobs")).await.unwrap();

    // Test operations...

    // temp_dir cleaned up automatically
}
```
