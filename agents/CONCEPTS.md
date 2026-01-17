# Key Concepts

High-level architecture and concepts for understanding jax-bucket.

## Overview

jax-bucket organizes data into **Buckets** - encrypted containers that hold files and directories. Each bucket has:

- A unique identifier (UUID)
- A friendly name
- Encrypted content (files, directories)
- Access control (who can read/write)
- Version history (immutable chain)

## Architecture Layers

```
┌─────────────────────────────────────────────────────────────┐
│                        CLI / HTTP API                        │
├─────────────────────────────────────────────────────────────┤
│                           Mount                              │
│              (Virtual filesystem abstraction)                │
├─────────────────────────────────────────────────────────────┤
│                          Manifest                            │
│           (Metadata, shares, version tracking)               │
├─────────────────────────────────────────────────────────────┤
│                          Crypto                              │
│        (Encryption, key sharing, authentication)             │
├─────────────────────────────────────────────────────────────┤
│                        BlobsStore                            │
│              (Content-addressed storage)                     │
├─────────────────────────────────────────────────────────────┤
│                           Peer                               │
│                (P2P networking via iroh)                     │
└─────────────────────────────────────────────────────────────┘
```

---

## Core Concepts

### Bucket

A bucket is the top-level container for your data. It's similar to a folder or repository, but encrypted and distributed.

- Each bucket has a **Secret** - a 256-bit key that encrypts all content
- The secret is never stored directly; it's split into **Shares** for each owner
- Buckets sync automatically between peers that have access

### Mount

A mount is the runtime representation of a bucket. When you "mount" a bucket, you:

1. Load the bucket's manifest from storage
2. Decrypt the secret using your share (or `published_secret` if mirror)
3. Gain access to the virtual filesystem

The mount provides file operations: `add`, `ls`, `cat`, `mkdir`, `mv`, `rm`.

### Manifest

The manifest is the root metadata for a bucket:

```rust
struct Manifest {
    id: Uuid,                           // Global unique identifier
    name: String,                       // Friendly display name
    shares: BTreeMap<String, Share>,    // Access control (pubkey -> Share)
    entry: Link,                        // Pointer to root node
    pins: Link,                         // Pinned content hashes
    previous: Option<Link>,             // Link to previous version
    height: u64,                        // Version chain height
    published_secret: Option<Secret>,   // Plaintext secret when published
}
```

Each modification creates a new manifest with a `previous` link, forming an immutable version history.

---

## Access Control

### PrincipalRole

Every principal (user) in a bucket has a role:

```rust
enum PrincipalRole {
    Owner,   // Full read/write access
    Mirror,  // Read-only after publication
}
```

### Share

A `Share` represents a principal's access to a bucket:

```rust
struct Share {
    principal: Principal,       // Identity (pubkey) and role
    share: Option<SecretShare>, // Encrypted key (owners only)
}
```

**Owners** always have a `SecretShare` - the bucket secret encrypted to their public key.

**Mirrors** never have individual shares - they use `published_secret` from the manifest after publication.

### Publishing

Publishing makes a bucket readable by mirrors:

```rust
fn publish(&mut self, secret: &Secret) {
    self.published_secret = Some(secret.clone());
}
```

- **Before publish**: Only owners can decrypt (via their `SecretShare`)
- **After publish**: Anyone with the manifest can decrypt (via `published_secret`)
- **Permanent**: Once published, the secret is exposed and cannot be revoked

This enables mirrors to sync encrypted blobs before publication, then decrypt once published.

---

## Content Structure

### Node

Bucket content is organized as a DAG (Directed Acyclic Graph) of nodes:

```rust
struct Node {
    links: BTreeMap<String, NodeLink>,
}
```

Each node maps names to either files or subdirectories.

### NodeLink

```rust
enum NodeLink {
    Data(Link, Secret, Data),  // File: hash, encryption key, metadata
    Dir(Link, Secret),         // Directory: hash, encryption key
}
```

**Key insight**: Every file and directory has its own unique `Secret`. This enables:
- Fine-grained encryption
- Efficient key rotation (change one item's key without re-encrypting everything)
- Stable content-addressed hashes

### Data Flow

```
Manifest.entry (Link)
    └─→ Root Node (encrypted with root Secret)
            ├─ "file.txt" → Data(Link, Secret, metadata)
            │                    └─→ Encrypted file content
            └─ "subdir/" → Dir(Link, Secret)
                               └─→ Child Node (encrypted)
                                       └─ ...
```

---

## Cryptography

### Identity (Ed25519)

Each peer has an Ed25519 keypair:
- **SecretKey**: 32 bytes, never shared
- **PublicKey**: 32 bytes, used as peer identity

### Content Encryption (ChaCha20-Poly1305)

All bucket content is encrypted with ChaCha20-Poly1305 (256-bit keys):

```
[nonce (12 bytes)] || [encrypted(hash || plaintext)] || [tag (16 bytes)]
```

- Random nonce per encryption
- BLAKE3 hash prepended before encryption for integrity

### Key Sharing (ECDH + AES-KW)

To share a bucket's secret with another peer:

1. Generate ephemeral Ed25519 keypair
2. Convert keys to X25519 (Montgomery curve) for ECDH
3. Perform ECDH to derive shared secret
4. Wrap bucket secret with AES-KW (RFC 3394)
5. Return: `[ephemeral_pubkey (32 bytes)] || [wrapped_secret (40 bytes)]`

The recipient reverses the process using their private key.

---

## Storage

### BlobsStore

Content-addressed storage via iroh-blobs. All data is identified by its BLAKE3 hash:

```rust
let hash = blobs.put(data).await?;  // Store, get hash
let data = blobs.get(&hash).await?; // Retrieve by hash
```

Benefits:
- Automatic deduplication
- Integrity verification
- Immutable history (old versions remain accessible)

### Links (CID)

References use **Links** (Content Identifiers):

```rust
struct Link(Cid);  // Wrapper around CID
```

- BLAKE3 hash only
- Two codecs: RAW (encrypted blobs) and DAG-CBOR (structured data)

---

## Networking

### Peer

jax-bucket uses iroh for P2P networking:

```rust
struct Peer {
    blobs_store: BlobsStore,    // Content-addressed storage
    secret_key: SecretKey,      // Peer identity
    endpoint: Endpoint,         // Network endpoint
    sync_provider: SyncProvider,// Background sync
}
```

### Sync

Peers sync buckets by:

1. Advertising bucket manifests they have
2. Requesting manifests and blobs from peers
3. Verifying integrity via content addressing
4. Resolving versions via manifest chain (height, previous links)

Sync happens automatically on daemon start, local changes, and peer connections.
