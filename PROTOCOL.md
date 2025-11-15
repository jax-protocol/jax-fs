# JaxBucket Protocol Specification

This document describes the technical details of the JaxBucket protocol, including data structures, cryptography, and synchronization mechanisms.

## Table of Contents

- [Overview](#overview)
- [Data Model](#data-model)
  - [Buckets](#buckets)
  - [Manifests](#manifests)
  - [Nodes](#nodes)
  - [Pins](#pins)
- [Cryptography](#cryptography)
  - [Identity](#identity)
  - [Key Sharing](#key-sharing)
  - [Content Encryption](#content-encryption)
- [Peer Structure](#peer-structure)
- [Synchronization Protocol](#synchronization-protocol)
- [Security Model](#security-model)

## Overview

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

## Data Model

### Buckets

A **bucket** is a versioned, encrypted collection of files and directories. Each bucket is identified by a **UUID** and contains:

- **Manifest**: Current state of the bucket (unencrypted metadata)
- **Root Node**: Encrypted directory structure
- **Blobs**: Encrypted file contents
- **Version Chain**: Link to previous manifest version

Buckets form a **version history** where each manifest points to its predecessor, creating an immutable audit trail.

### Manifests

The manifest is the entry point to a bucket. It contains unencrypted metadata about the bucket's structure and access control.

**Location**: `rust/crates/common/src/bucket/manifest.rs:54`

```rust
pub struct Manifest {
    pub id: Uuid,                    // Global bucket identifier
    pub name: String,                // Display name (not unique)
    pub shares: Shares,              // Access control list
    pub entry: Link,                 // Points to root Node
    pub pins: Link,                  // Points to Pins (HashSeq)
    pub previous: Option<Link>,      // Previous manifest version
    pub version: Version,            // Software version metadata
}
```

**Key Fields:**

- **`id`**: UUID that uniquely identifies this bucket across all peers
- **`name`**: Human-readable label (can be changed, not guaranteed unique)
- **`shares`**: Map of `PublicKey -> BucketShare` defining who can access the bucket
- **`entry`**: Content-addressed link (CID) pointing to the encrypted root directory node
- **`pins`**: Link to a HashSeq containing all content hashes that should be kept locally
- **`previous`**: Link to the prior manifest version (forms version chain)
- **`version`**: Software version that created this manifest

**Serialization:**
- Manifests are serialized using **DAG-CBOR** (IPLD)
- Stored as raw blobs in Iroh's BlobStore
- Addressed by their BLAKE3 hash

### Nodes

A **node** represents a directory in the bucket's file tree. Nodes are **encrypted** and **content-addressed**.

**Location**: `rust/crates/common/src/bucket/node.rs`

```rust
pub struct Node {
    pub links: BTreeMap<String, NodeLink>,
}

pub enum NodeLink {
    Data(Link, Secret, Metadata),  // File
    Dir(Link, Secret),             // Subdirectory
}

pub struct Metadata {
    pub mime_type: Option<String>,
    pub custom: BTreeMap<String, String>,
}
```

**Structure:**

- **`links`**: Sorted map of name → NodeLink
  - Keys are file/directory names (e.g., `"README.md"`, `"src"`)
  - Values describe the target (file or subdirectory)

**NodeLink Variants:**

1. **`Data(link, secret, metadata)`**: Represents a file
   - `link`: Content-addressed pointer to encrypted file blob
   - `secret`: Encryption key for decrypting the file
   - `metadata`: MIME type and custom properties

2. **`Dir(link, secret)`**: Represents a subdirectory
   - `link`: Content-addressed pointer to child Node
   - `secret`: Encryption key for decrypting the child Node

**Encryption:**

1. Node is serialized to DAG-CBOR
2. Encrypted with ChaCha20-Poly1305 using the node's secret key
3. Stored as a blob
4. Addressed by BLAKE3 hash of the ciphertext

**Example:**

```text
Root Node (encrypted with bucket secret):
{
  "README.md": Data(QmABC..., [secret], {mime: "text/markdown"}),
  "src":       Dir(QmXYZ..., [secret])
}
  └─> src Node (encrypted with its own secret):
      {
        "main.rs": Data(QmDEF..., [secret], {mime: "text/rust"}),
        "lib.rs":  Data(QmGHI..., [secret], {mime: "text/rust"})
      }
```

### Pins

**Pins** define which content should be kept locally. They prevent garbage collection of important blobs.

**Location**: `rust/crates/common/src/bucket/pins.rs`

```rust
pub struct Pins(pub HashSet<Hash>);
```

**Format:**
- Set of BLAKE3 hashes representing blobs to keep
- Serialized as an Iroh **HashSeq** (ordered list of hashes)
- Stored as a blob, linked from the manifest

**Usage:**

When saving a bucket:
1. Collect all Node and file blob hashes
2. Add them to the Pins set
3. Serialize as HashSeq and store
4. Manifest's `pins` field points to this HashSeq

When syncing:
1. Download the pins HashSeq
2. Verify all pinned content is available
3. Download missing blobs from peers

### Bucket Log

The **bucket log** is a height-based version control system that tracks all versions of a bucket, including divergent forks. It enables efficient synchronization and conflict resolution across peers.

**Location**: `rust/crates/common/src/bucket_log/`

**Structure:**

Each peer maintains a local log mapping `bucket_id → height → Vec<Link>`:

```rust
pub trait BucketLogProvider {
    // Get all heads at a specific height (may have multiple if forked)
    async fn heads(&self, id: Uuid, height: u64) -> Result<Vec<Link>>;

    // Append a new version to the log
    async fn append(
        &self,
        id: Uuid,
        name: String,
        current: Link,
        previous: Option<Link>,
        height: u64,
    ) -> Result<()>;

    // Get the maximum height for a bucket
    async fn height(&self, id: Uuid) -> Result<u64>;

    // Check if a link exists and return all heights where it appears
    async fn has(&self, id: Uuid, link: Link) -> Result<Vec<u64>>;

    // Get the canonical head (max link if multiple heads at same height)
    async fn head(&self, id: Uuid, height: Option<u64>) -> Result<(Link, u64)>;
}
```

**Key Concepts:**

1. **Height**: Monotonically increasing version number
   - Genesis manifests have `height = 0`, `previous = None`
   - Each subsequent version has `height = parent_height + 1`
   - Height determines ordering in the version DAG

2. **Multiple Heads**: Forks are represented as multiple links at the same height
   - When peers make concurrent edits, both versions are recorded at the same height
   - The `head()` function selects the **maximum link** (by hash comparison) as canonical
   - This provides deterministic conflict resolution across all peers

3. **DAG Structure**: Manifests form a directed acyclic graph
   - Each manifest's `previous` field points to its parent
   - The log validates that `previous` exists at `height - 1` before appending
   - This creates a verifiable chain back to genesis

**Example Log:**

```text
bucket_id: 550e8400-e29b-41d4-a716-446655440000

height 0: [Link(QmGenesis...)]           ← Genesis
          ↓
height 1: [Link(QmFirst...)]             ← Linear history
          ↓
height 2: [Link(QmSecond...)]
          ↓
height 3: [Link(QmAlice...), Link(QmBob...)]  ← Fork! Two concurrent edits
          ↓          ↓
height 4: [Link(QmMerge...)]             ← Converged (both Alice and Bob sync)

head() at height 3 returns max(QmAlice, QmBob) → deterministic selection
```

**Validation Rules:**

When appending a new log entry:

1. **Height Validation**: If `previous` is provided, it must exist at `height - 1`
2. **Genesis Rule**: If `previous = None`, then `height` must be 0
3. **Conflict Detection**: Same link cannot appear twice at the same height
4. **Provenance** (during sync): Peer providing the update must be in the manifest's shares

**Sync Integration:**

The log enables efficient synchronization:

1. **Height Comparison**: Peers exchange heights during ping to detect divergence
2. **Ancestor Finding**: Walk back the manifest chain to find common ancestor
   - Check if each `previous` link exists in local log using `has()`
   - Stop when found or reach genesis
3. **Chain Download**: Download manifests from target back to common ancestor
4. **Log Application**: Append downloaded manifests to local log
   - Heights are validated during append
   - Forks are automatically detected and stored

This design supports **eventual consistency** - all peers converge to the same canonical head through deterministic fork resolution, while preserving the complete version history including divergent branches.

## Cryptography

### Identity

Each peer has an **Ed25519 keypair** as their identity.

**Location**: `rust/crates/common/src/crypto/keys.rs`

```rust
pub struct SecretKey(ed25519_dalek::SigningKey);  // 32 bytes
pub struct PublicKey(ed25519_dalek::VerifyingKey); // 32 bytes
```

**Properties:**
- **SecretKey**: Stored in `~/.config/jax/secret.pem` (PEM format)
- **PublicKey**: Derived from secret key, used as Node ID
- **Dual Purpose**:
  1. Network identity (Iroh uses PublicKey as NodeId)
  2. Encryption key sharing (converted to X25519 for ECDH)

**Key Generation:**
```rust
let secret_key = SecretKey::generate();
let public_key = secret_key.public_key();
```

### Key Sharing

Buckets are shared between peers using **ECDH + AES Key Wrap**.

**Location**: `rust/crates/common/src/crypto/share.rs`

**Protocol:**

To share a bucket secret with another peer:

1. **Generate Ephemeral Key**: Create temporary Ed25519 keypair
2. **ECDH**: Convert both keys to X25519 and compute shared secret
   ```rust
   let shared_secret = ecdh(ephemeral_secret, recipient_public);
   ```
3. **AES Key Wrap**: Wrap the bucket secret using shared secret (RFC 3394)
   ```rust
   let wrapped = aes_kw::wrap(kek: shared_secret, secret: bucket_secret);
   ```
4. **Package Share**: Combine ephemeral public key + wrapped secret
   ```rust
   Share = [ephemeral_pubkey(32 bytes) || wrapped_secret(40 bytes)]
   // Total: 72 bytes
   ```

**Unwrapping:**

The recipient recovers the secret:

1. Extract ephemeral public key from Share (first 32 bytes)
2. Compute ECDH with their private key
   ```rust
   let shared_secret = ecdh(my_secret, ephemeral_public);
   ```
3. Unwrap the secret using AES-KW
   ```rust
   let bucket_secret = aes_kw::unwrap(kek: shared_secret, wrapped);
   ```

**BucketShare Structure:**

```rust
pub struct BucketShare {
    pub principal: Principal,
    pub share: Share,
}

pub struct Principal {
    pub role: PrincipalRole,  // Owner, Editor, Viewer
    pub identity: PublicKey,  // Peer's public key
}

pub enum PrincipalRole {
    Owner,   // Full control
    Editor,  // Read + Write
    Viewer,  // Read only
}
```

### Content Encryption

Files and nodes are encrypted with **ChaCha20-Poly1305 AEAD**.

**Location**: `rust/crates/common/src/crypto/secret.rs`

```rust
pub struct Secret([u8; 32]);  // 256-bit key
```

**Encryption Process:**

1. **Generate Nonce**: Random 96-bit nonce (12 bytes)
2. **Encrypt**: Use ChaCha20-Poly1305
   ```rust
   let cipher = ChaCha20Poly1305::new(&secret);
   let ciphertext = cipher.encrypt(&nonce, plaintext)?;
   ```
3. **Format**: `nonce(12) || ciphertext || tag(16)`
4. **Hash**: Compute BLAKE3 hash of the encrypted blob
5. **Store**: Save blob with hash as address

**Properties:**
- **Per-Item Keys**: Each file and node has its own Secret
- **Content Addressing**: Hashes are stable (computed after encryption)
- **Fine-Grained Access**: Can share individual file keys without exposing entire bucket
- **Authentication**: AEAD provides tamper detection

**Decryption:**

1. Extract nonce (first 12 bytes)
2. Decrypt remaining bytes
   ```rust
   let plaintext = cipher.decrypt(&nonce, ciphertext)?;
   ```
3. Verify AEAD tag (automatic, failure = tampered data)

## Peer Structure

A JaxBucket peer consists of:

### 1. Identity

**Ed25519 keypair** stored in `secret.pem`:
- Private key for decryption and signing
- Public key serves as Node ID

### 2. BlobStore

**Iroh's content-addressed storage**:
- Stores encrypted nodes and files
- Deduplicates by BLAKE3 hash
- Supports Raw blobs and HashSeq collections
- Local cache on disk

**Location**: `~/.config/jax/blobs/`

### 3. Endpoint

**Iroh's QUIC networking**:
- NAT traversal via STUN/TURN
- DHT-based peer discovery (Mainline DHT)
- Multiple ALPN protocols:
  - `iroh-blobs`: For blob transfer
  - `jax-protocol`: For sync messages

### 4. Database

**SQLite database** for metadata:
- Bucket manifests
- Current bucket links
- Sync status
- Peer relationships

**Location**: `~/.config/jax/jax.db`

## Synchronization Protocol

JaxBucket implements a pull-based P2P sync protocol using height-based version comparison. Peers discover divergence through periodic pings and pull missing manifest chains to converge.

**Architecture**: Queue-based sync provider with background job processing
**Protocol**: Custom QUIC/bincode messages over Iroh
**ALPN**: `/iroh-jax/1`

### Sync Architecture

**Location**: `rust/crates/app/src/daemon/sync_provider.rs`

JaxBucket uses a **QueuedSyncProvider** that decouples protocol handlers from sync execution:

```rust
pub struct QueuedSyncProvider {
    job_queue: Sender<SyncJob>,     // flume channel (default capacity: 1000)
    peer: Peer,                      // Iroh networking peer
    log_provider: BucketLogProvider, // Height-based version log
}

pub enum SyncJob {
    SyncBucket { bucket_id, peer_id },  // Download manifests and update log
    DownloadPins { bucket_id, link },   // Download pinned content
    PingPeer { bucket_id, peer_id },    // Check sync status
}
```

**Background Worker**:
- Processes jobs from the queue asynchronously
- Runs periodic ping scheduler every 60 seconds
- Provides backpressure via bounded channel (prevents memory exhaustion)

**Benefits**:
- Protocol handlers respond immediately (no blocking on I/O)
- Sync operations isolated from network layer
- Failed jobs don't crash protocol handlers
- Easy to implement different execution strategies (sync, queued, actor-based)

### Protocol Messages

**Location**: `rust/crates/common/src/peer/protocol/messages/ping.rs`

JaxBucket uses a **bidirectional request/response pattern** where both the initiator and responder can trigger sync jobs as side effects.

#### Ping/Pong

Compare bucket versions and trigger sync if divergent:

```rust
// Initiator sends:
PingMessage {
    bucket_id: Uuid,
    our_link: Link,     // Our current head link
    our_height: u64,    // Our current height
}

// Responder replies:
PingReply {
    status: PingReplyStatus,
}

enum PingReplyStatus {
    NotFound,                  // Responder doesn't have this bucket
    Ahead(Link, u64),          // Responder ahead: (their_link, their_height)
    Behind(Link, u64),         // Responder behind: (their_link, their_height)
    InSync,                    // Same height (may still have different heads)
}
```

**Message Flow**:

```text
Initiator (Peer A)              Responder (Peer B)
    |                                |
    |  PingMessage                   |
    |  - bucket_id                   |
    |  - our_link                    |
    |  - our_height: 5               |
    |------------------------------>|
    |                                |  1. Compare heights
    |                                |     their_height: 5
    |                                |     our_height: 7
    |                                |
    |  PingReply                     |  2. Generate response
    |  - Ahead(link_7, 7)            |
    |<------------------------------|
    |                                |  3. Side effect (async):
    |  4. Handle reply:              |     Behind → dispatch SyncBucket
    |     Ahead → dispatch SyncBucket|
    |                                |
```

**Bidirectional Sync Triggering**:

Both sides independently decide whether to sync:

| Initiator Height | Responder Height | Initiator Action | Responder Action |
|-----------------|-----------------|------------------|------------------|
| 5 | 7 | Dispatch SyncBucket | No action |
| 7 | 5 | No action | Dispatch SyncBucket |
| 5 | 5 | No action | No action |
| - | 3 | Dispatch SyncBucket | Dispatch SyncBucket |

**Side Effects Pattern**:

The protocol uses a `BidirectionalHandler` trait that separates response generation from side effects:

```rust
trait BidirectionalHandler {
    // Responder: generate immediate response
    async fn handle_message(&self, msg: Message) -> Reply;

    // Responder: async side effects after response sent
    async fn handle_message_side_effect(&self, msg: Message, reply: Reply);

    // Initiator: process response and trigger actions
    async fn handle_reply(&self, msg: Message, reply: Reply);
}
```

This ensures:
- Fast response times (no blocking on I/O)
- Both sides can trigger sync jobs independently
- Failed side effects don't prevent response delivery

### Sync Workflow

**Location**: `rust/crates/common/src/peer/sync/jobs/sync_bucket.rs`

#### Height-Based Sync Process

When a peer discovers it's behind (via ping):

```text
1. TRIGGER
   ├─ Periodic ping (every 60s)
   └─ Manual save_mount() → immediate ping to all peers in shares

2. PING EXCHANGE
   ├─ Compare our_height vs their_height
   └─ If behind: dispatch SyncBucketJob(bucket_id, peer_id)

3. SYNC EXECUTION (sync_bucket::execute)
   │
   ├─ a. Check if bucket exists locally
   │     ├─ Yes: get our current (link, height) from log
   │     └─ No: set current = None (will download full chain)
   │
   ├─ b. Find common ancestor
   │     ├─ Download peer's current manifest
   │     ├─ Walk backward via previous links
   │     ├─ For each manifest:
   │     │   ├─ Check if link exists in our log (via has())
   │     │   └─ If found: this is the common ancestor
   │     └─ Stop at common ancestor or genesis
   │
   ├─ c. Download manifest chain
   │     ├─ Collect all manifests from target back to ancestor
   │     ├─ Download each manifest blob from peer
   │     └─ Build chain: [ancestor+1, ancestor+2, ..., target]
   │
   ├─ d. Verify provenance
   │     └─ Check our public key is in final manifest.shares
   │
   ├─ e. Apply manifest chain to log
   │     ├─ For each manifest in chain:
   │     │   ├─ Extract (link, previous, height)
   │     │   ├─ Call log.append(id, name, link, previous, height)
   │     │   └─ Log validates: previous exists at height-1
   │     └─ Update canonical head
   │
   └─ f. Download pinned content
         └─ Dispatch DownloadPinsJob(bucket_id, target_link)
```

**Common Ancestor Finding**:

```rust
async fn find_common_ancestor(
    peer_id: PublicKey,
    bucket_id: Uuid,
    their_link: Link,
    our_current: Option<(Link, u64)>,
) -> Result<Option<Link>> {
    let mut cursor = their_link;

    loop {
        let manifest = download_manifest(peer_id, cursor).await?;

        // Check if we have this link in our log
        let heights = log.has(bucket_id, cursor).await?;
        if !heights.is_empty() {
            return Ok(Some(cursor));  // Found common ancestor
        }

        // Walk backward
        match manifest.previous {
            Some(prev) => cursor = prev,
            None => return Ok(None),  // Reached genesis, no common ancestor
        }
    }
}
```

**Sync Properties**:

- **Pull-based**: Peers only pull updates when they discover they're behind
- **No push announcements**: Removed for simplicity (peers discover via ping)
- **Eventual consistency**: All peers converge to same canonical head via deterministic fork resolution
- **Fork tolerance**: Multiple concurrent edits create multiple heads at same height
- **Bounded walks**: Ancestor finding walks backward until found (not bounded by depth limit)

### Sync Verification

**Location**: `rust/crates/common/src/peer/sync/jobs/sync_bucket.rs`

Core verification checks applied during sync:

#### 1. Provenance Check

Only accept updates from authorized peers:

```rust
// After downloading manifest chain, check final manifest
let final_manifest = chain.last().unwrap();
if !final_manifest.shares.contains_key(&our_public_key) {
    return Err("Unauthorized: we are not in bucket shares");
}
```

This prevents:
- Unauthorized peers from injecting manifests
- Accidental sync of buckets we don't have access to

#### 2. Height Validation

The bucket log enforces structural integrity when appending:

```rust
// In BucketLogProvider::append()
if let Some(previous_link) = previous {
    // Non-genesis: previous must exist at height - 1
    if height == 0 {
        return Err("Invalid: height 0 with previous link");
    }

    let expected_height = height - 1;
    let prev_exists = log.heads(bucket_id, expected_height)
        .await?
        .contains(&previous_link);

    if !prev_exists {
        return Err("Invalid: previous link not found at height-1");
    }
} else {
    // Genesis: must be height 0
    if height != 0 {
        return Err("Invalid: non-zero height with no previous");
    }
}
```

This ensures:
- Manifests form a valid DAG structure
- Heights are sequential and consistent
- No orphaned manifests (all non-genesis have valid parent)

#### 3. Fork Detection and Resolution

When multiple heads exist at the same height:

```rust
// Get canonical head (deterministic across all peers)
let (canonical_link, height) = log.head(bucket_id, None).await?;

// head() selects max link by hash comparison
async fn head(&self, id: Uuid) -> Result<(Link, u64)> {
    let height = self.height(id).await?;
    let heads = self.heads(id, height).await?;

    let canonical = heads.into_iter()
        .max()  // Compare links by hash (deterministic)
        .ok_or(HeadNotFound)?;

    Ok((canonical, height))
}
```

This provides:
- **Deterministic convergence**: All peers select same canonical head
- **Fork preservation**: All versions retained in log (for audit/debugging)
- **Automatic conflict resolution**: No manual intervention required

### Periodic Sync Coordination

**Location**: `rust/crates/app/src/daemon/sync_provider.rs:run_worker()`

The background worker runs a periodic ping scheduler:

```rust
loop {
    tokio::select! {
        // Process sync jobs from queue
        Some(job) = job_rx.recv() => {
            match job {
                SyncJob::SyncBucket { .. } => execute_sync_bucket(...).await,
                SyncJob::DownloadPins { .. } => execute_download_pins(...).await,
                SyncJob::PingPeer { .. } => execute_ping(...).await,
            }
        }

        // Periodic ping all peers (every 60 seconds)
        _ = interval.tick() => {
            let buckets = log.list_buckets().await?;
            for bucket_id in buckets {
                for peer_id in get_bucket_peers(bucket_id).await? {
                    dispatch_job(SyncJob::PingPeer { bucket_id, peer_id });
                }
            }
        }
    }
}
```

**Trigger Points**:

1. **Periodic**: Background scheduler pings all peers every 60 seconds
2. **On-demand**: `save_mount()` immediately pings all peers in bucket.shares
3. **Reactive**: Incoming ping from peer can trigger sync job as side effect

This ensures:
- Timely discovery of updates (within 60s)
- Immediate propagation when local edits made
- Bidirectional sync (both sides can detect divergence)

## Security Model

### Threat Model

**JaxBucket protects against:**

✅ **Untrusted Storage Providers**
- All blobs are encrypted
- Storage provider sees only hashes
- Cannot decrypt content without keys

✅ **Passive Network Observers**
- QUIC provides TLS 1.3 encryption
- Peer connections are authenticated
- Traffic is encrypted end-to-end

✅ **Unauthorized Peers**
- Only peers with valid BucketShare can decrypt
- ECDH ensures only recipient can unwrap secrets
- Access control enforced via shares list

✅ **Tampered Data**
- AEAD detects modifications
- Content addressing ensures integrity
- Hash verification on all blobs

**JaxBucket does NOT protect against:**

❌ **Compromised Peer with Valid Access**
- If an authorized peer is compromised, attacker gains access
- No forward secrecy or key rotation (yet)
- Recommendation: Regularly audit shares list

❌ **Malicious Authorized Peer**
- Authorized peers can leak data
- Trust model assumes peers with access are trustworthy
- Recommendation: Only share with trusted devices/users

❌ **Metadata Leakage**
- Bucket structure visible (file count, sizes, hierarchy)
- Storage provider can see blob access patterns
- Recommendation: Use padding or cover traffic (future work)

❌ **Traffic Analysis**
- Connection patterns may reveal peer relationships
- Sync frequency might leak activity patterns
- Recommendation: Use Tor or mixnets (future work)

❌ **Side-Channel Attacks**
- Timing attacks on crypto operations
- Power analysis (if physical access)
- Recommendation: Use constant-time crypto (mostly implemented)

### Best Practices

1. **Protect Secret Keys**
   - Store `secret.pem` with `chmod 600`
   - Back up securely (encrypted, offline)
   - Never share or commit to version control

2. **Verify Peer Identity**
   - Check public key fingerprints out-of-band
   - Use QR codes or secure channels for initial sharing

3. **Regular Key Rotation**
   - Periodically rotate bucket secrets (manual process currently)
   - Remove old shares when no longer needed

4. **Audit Access**
   - Regularly review bucket shares
   - Remove peers that no longer need access

5. **Monitor Sync Activity**
   - Watch for unexpected updates
   - Investigate unknown peers or sync patterns

### Future Security Enhancements

- [ ] Forward secrecy via key rotation
- [ ] Access revocation with re-encryption
- [ ] Metadata padding to hide structure
- [ ] Traffic obfuscation
- [ ] Formal security audit

## Implementation Details

### Key Files

**Data Model:**
- **Manifest**: `rust/crates/common/src/bucket/manifest.rs`
- **Node**: `rust/crates/common/src/bucket/node.rs`
- **Pins**: `rust/crates/common/src/bucket/pins.rs`
- **Bucket Log**: `rust/crates/common/src/bucket_log/`
  - `provider.rs` - BucketLogProvider trait definition
  - `memory.rs` - In-memory implementation (testing/minimal peers)
  - Database implementations in app-specific crates

**Cryptography:**
- **Keys**: `rust/crates/common/src/crypto/keys.rs`
- **Secret**: `rust/crates/common/src/crypto/secret.rs`
- **Share**: `rust/crates/common/src/crypto/share.rs`

**Peer & Sync:**
- **Link**: `rust/crates/common/src/linked_data/link.rs`
- **Peer**: `rust/crates/common/src/peer/mod.rs`
- **Protocol Messages**: `rust/crates/common/src/peer/protocol/messages/`
  - `ping.rs` - Ping/Pong message definitions
  - `mod.rs` - Message router and handler registration
- **Sync Jobs**: `rust/crates/common/src/peer/sync/jobs/`
  - `sync_bucket.rs` - Manifest chain download and log application
  - `download_pins.rs` - Pin content download
  - `mod.rs` - Job definitions and common utilities
- **Sync Provider**: `rust/crates/app/src/daemon/sync_provider.rs`
  - QueuedSyncProvider implementation
  - Background worker with periodic ping scheduler

### Dependencies

- **Iroh**: P2P networking and blob storage
- **ed25519-dalek**: Identity keypairs
- **chacha20poly1305**: Content encryption
- **aes-kw**: Key wrapping (RFC 3394)
- **blake3**: Content addressing (via Iroh)
- **serde_ipld_dagcbor**: DAG-CBOR serialization

## References

- **Iroh**: https://iroh.computer/
- **IPLD**: https://ipld.io/
- **RFC 3394** (AES Key Wrap): https://tools.ietf.org/html/rfc3394
- **ChaCha20-Poly1305**: https://tools.ietf.org/html/rfc8439
- **Ed25519**: https://ed25519.cr.yp.to/
- **BLAKE3**: https://github.com/BLAKE3-team/BLAKE3
