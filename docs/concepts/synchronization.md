# Synchronization

This document describes JaxBucket's peer structure and synchronization protocol.

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

**Location**: `crates/daemon/src/daemon/sync_provider.rs`

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

**Location**: `crates/common/src/peer/protocol/messages/ping.rs`

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
    |  4. Handle reply:              |     Behind -> dispatch SyncBucket
    |     Ahead -> dispatch SyncBucket|
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

**Location**: `crates/common/src/peer/sync/jobs/sync_bucket.rs`

#### Height-Based Sync Process

When a peer discovers it's behind (via ping):

```text
1. TRIGGER
   ├─ Periodic ping (every 60s)
   └─ Manual save_mount() -> immediate ping to all peers in shares

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

**Location**: `crates/common/src/peer/sync/jobs/sync_bucket.rs`

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

**Location**: `crates/daemon/src/daemon/sync_provider.rs:run_worker()`

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
