# Data Model

This document describes the core data structures in JaxBucket: Buckets, Manifests, Nodes, Pins, and the Bucket Log.

## Buckets

A **bucket** is a versioned, encrypted collection of files and directories. Each bucket is identified by a **UUID** and contains:

- **Manifest**: Current state of the bucket (unencrypted metadata)
- **Root Node**: Encrypted directory structure
- **Blobs**: Encrypted file contents
- **Version Chain**: Link to previous manifest version

Buckets form a **version history** where each manifest points to its predecessor, creating an immutable audit trail.

## Manifests

The manifest is the entry point to a bucket. It contains unencrypted metadata about the bucket's structure and access control.

**Location**: `crates/common/src/bucket/manifest.rs`

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

## Nodes

A **node** represents a directory in the bucket's file tree. Nodes are **encrypted** and **content-addressed**.

**Location**: `crates/common/src/bucket/node.rs`

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

- **`links`**: Sorted map of name -> NodeLink
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

## Pins

**Pins** define which content should be kept locally. They prevent garbage collection of important blobs.

**Location**: `crates/common/src/bucket/pins.rs`

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

## Bucket Log

The **bucket log** is a height-based version control system that tracks all versions of a bucket, including divergent forks. It enables efficient synchronization and conflict resolution across peers.

**Location**: `crates/common/src/bucket_log/`

### Structure

Each peer maintains a local log mapping `bucket_id -> height -> Vec<Link>`:

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

### Key Concepts

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

### Example Log

```text
bucket_id: 550e8400-e29b-41d4-a716-446655440000

height 0: [Link(QmGenesis...)]           <- Genesis
          |
height 1: [Link(QmFirst...)]             <- Linear history
          |
height 2: [Link(QmSecond...)]
          |
height 3: [Link(QmAlice...), Link(QmBob...)]  <- Fork! Two concurrent edits
          |          |
height 4: [Link(QmMerge...)]             <- Converged (both Alice and Bob sync)

head() at height 3 returns max(QmAlice, QmBob) -> deterministic selection
```

### Validation Rules

When appending a new log entry:

1. **Height Validation**: If `previous` is provided, it must exist at `height - 1`
2. **Genesis Rule**: If `previous = None`, then `height` must be 0
3. **Conflict Detection**: Same link cannot appear twice at the same height
4. **Provenance** (during sync): Peer providing the update must be in the manifest's shares

### Sync Integration

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
