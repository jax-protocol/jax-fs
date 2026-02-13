# FUSE Architecture

This document describes how the FUSE filesystem integration works in jax-bucket.

## Overview

FUSE (Filesystem in Userspace) allows mounting buckets as local directories. Users can read and write files using standard filesystem tools (ls, cat, cp, etc.) without knowing about the underlying bucket/sync infrastructure.

## Components

```
┌─────────────────────────────────────────────────────────────────┐
│                         Kernel (FUSE)                           │
│                                                                 │
│  ls /mnt/bucket    cat /mnt/bucket/file.txt    echo > file.txt │
└─────────────────────────────────┬───────────────────────────────┘
                                  │ FUSE protocol
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                       JaxFs (fuser crate)                       │
│                                                                 │
│  Implements fuser::Filesystem trait                             │
│  Translates FUSE ops to Mount operations                        │
│                                                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │ InodeTable  │  │  FileCache  │  │  WriteBuffers           │  │
│  │ path ↔ ino  │  │  LRU + TTL  │  │  fh → pending data      │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
│                                                                 │
│  save_tx ──────────────────────────────────────────────────────►│
└─────────────────────────────────┬───────────────────────────────┘
                                  │ Direct Rust calls
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                          Mount                                  │
│                                                                 │
│  In-memory representation of bucket state                       │
│  - entry: Node (directory tree)                                 │
│  - ops_log: PathOpLog (CRDT operations)                         │
│  - manifest: Manifest metadata                                  │
│                                                                 │
│  Methods: ls(), cat(), add(), rm(), mkdir(), mv(), merge_from() │
└─────────────────────────────────┬───────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                        BlobsStore                               │
│                                                                 │
│  Content-addressed storage (iroh-blobs)                         │
│  All data encrypted with ChaCha20-Poly1305                      │
└─────────────────────────────────────────────────────────────────┘
```

## Key Files

| File | Purpose |
|------|---------|
| `crates/daemon/src/fuse/mod.rs` | Module exports |
| `crates/daemon/src/fuse/jax_fs.rs` | FUSE filesystem implementation |
| `crates/daemon/src/fuse/mount_manager.rs` | Mount lifecycle management |
| `crates/daemon/src/fuse/inode_table.rs` | Path ↔ inode mapping |
| `crates/daemon/src/fuse/cache.rs` | LRU content/attr cache |
| `crates/daemon/src/fuse/sync_events.rs` | Sync event and save request types |

## Data Flow

### Read Path

```
User: cat /mnt/bucket/docs/readme.md
                │
                ▼
┌─────────────────────────────────┐
│  FUSE read(ino, offset, size)   │
└─────────────────────────────────┘
                │
                ▼
┌─────────────────────────────────┐
│  InodeTable: ino → "/docs/readme.md"
└─────────────────────────────────┘
                │
                ▼
┌─────────────────────────────────┐
│  FileCache.get_content(path)    │
│                                 │
│  HIT? → Return cached data      │
│  MISS? → Continue...            │
└─────────────────────────────────┘
                │
                ▼
┌─────────────────────────────────┐
│  Mount.cat(path)                │
│                                 │
│  1. Traverse entry tree         │
│  2. Find NodeLink for path      │
│  3. Fetch blob from BlobsStore  │
│  4. Decrypt with node's Secret  │
│  5. Return plaintext bytes      │
└─────────────────────────────────┘
                │
                ▼
┌─────────────────────────────────┐
│  FileCache.put_content(path)    │
│  Return data to FUSE            │
└─────────────────────────────────┘
```

### Write Path

```
User: echo "hello" > /mnt/bucket/new.txt
                │
                ▼
┌─────────────────────────────────┐
│  FUSE create(parent, name)      │
│  → Returns file handle (fh)     │
└─────────────────────────────────┘
                │
                ▼
┌─────────────────────────────────┐
│  FUSE write(ino, fh, data)      │
│  → Buffers in WriteBuffers[fh]  │
│  → Marks dirty = true           │
└─────────────────────────────────┘
                │
                ▼
┌─────────────────────────────────┐
│  FUSE flush(ino, fh)            │
│                                 │
│  If dirty:                      │
│    Mount.add(path, buffer_data) │
│    - Encrypts data              │
│    - Stores blob                │
│    - Updates entry tree         │
│    - Records in ops_log         │
│    Mark clean                   │
│    Invalidate cache             │
│    Send SaveRequest ────────────┼──► MountManager
└─────────────────────────────────┘
                │
                ▼
┌─────────────────────────────────┐
│  FUSE release(ino, fh)          │
│  → Removes WriteBuffer[fh]      │
└─────────────────────────────────┘
```

### Write Persistence

After `flush()` succeeds, JaxFs sends a `SaveRequest` via channel to MountManager:

```
JaxFs.flush()
    │
    ├── mount.add(path, data)     // Updates in-memory state
    ├── cache.invalidate(path)
    └── save_tx.send(SaveRequest) // Request persistence
            │
            ▼
MountManager.spawn_save_handler()
    │
    └── peer.save_mount(&mount)   // Persists to manifest + notifies peers
            │
            ├── Saves manifest to blobs
            ├── Appends to bucket log
            └── Dispatches PingPeer jobs
```

### What `Mount.add()` Does

When FUSE calls `mount.add(path, data)`:

1. **Encrypt**: Generate new `Secret`, encrypt data with ChaCha20-Poly1305
2. **Store blob**: `blobs.put_stream(encrypted)` → returns content hash
3. **Create NodeLink**: Links hash + secret for decryption
4. **Update entry tree**: Traverse to parent, insert new NodeLink
5. **Store updated nodes**: Parent nodes stored to blobs
6. **Track pins**: Add all new hashes to `pins` set
7. **Record operation**: `ops_log.record(Add, path, link)`

## MountManager

The `MountManager` handles mount lifecycle and persistence:

```rust
pub struct MountManager {
    /// Live mounts: mount_id → LiveMount
    mounts: RwLock<HashMap<Uuid, LiveMount>>,
    /// Database for config persistence
    db: Database,
    /// Peer for loading/saving mounts
    peer: Peer<Database>,
    /// Event channel for sync notifications
    sync_tx: broadcast::Sender<SyncEvent>,
}

pub struct LiveMount {
    /// The bucket mount (kept alive)
    pub mount: Arc<RwLock<Mount>>,
    /// FUSE session handle
    pub session: Option<BackgroundSession>,
    /// File cache
    pub cache: FileCache,
    /// Config from database
    pub config: FuseMount,
}
```

### Lifecycle

```
mount start
    │
    ├── Load mount from peer: peer.mount(bucket_id)
    ├── Create FileCache
    ├── Create save channel (mpsc)
    ├── Create JaxFs with mount reference + save_tx
    ├── Spawn save handler task
    ├── Spawn fuser::BackgroundSession
    ├── Store in mounts HashMap
    └── Update DB status → Running

mount stop
    │
    ├── Take session (drops it, unmounts)
    ├── Platform unmount (umount/fusermount)
    └── Update DB status → Stopped

on_bucket_synced (called when sync completes)
    │
    ├── Find affected mounts
    ├── Check if ops_log has local changes
    │   │
    │   ├── If empty: Replace with fresh peer.mount()
    │   │
    │   └── If has changes: Merge with conflict resolution
    │       ├── Load incoming mount
    │       ├── mount.merge_from(incoming, ConflictFile)
    │       ├── Log resolved conflicts
    │       └── Save merged result
    │
    ├── Invalidate cache
    └── Emit SyncEvent::MountInvalidated
```

## Sync Integration

### Incoming Remote Changes

When a remote peer syncs new changes:

```
Remote peer announces new manifest
        │
        ▼
Sync engine downloads and applies manifest
        │
        ▼
MountManager.on_bucket_synced(bucket_id)
        │
        ▼
Check for local changes (ops_log.is_empty()?)
        │
        ├── No local changes:
        │   └── Replace mount with peer.mount()
        │
        └── Has local changes:
            ├── Load incoming: peer.mount(bucket_id)
            ├── Merge: mount.merge_from(incoming, ConflictFile)
            │   └── Creates conflict files for concurrent edits
            ├── Save: peer.save_mount(&mount)
            └── Log conflicts resolved
        │
        ▼
Invalidate cache + emit MountInvalidated
        │
        ▼
JaxFs sync listener receives event
  - Calls cache.invalidate_all()
  - Next FUSE read sees updated content
```

### Conflict Resolution

When concurrent edits occur, the `ConflictFile` resolver creates conflict copies:

```
Peer A: Creates /config.json with "version: 1"
Peer B: Creates /config.json with "version: 2"
        │
        ▼
Both sync to each other
        │
        ▼
merge_from() with ConflictFile resolver
        │
        ▼
Result:
  /config.json         ← One version (CRDT winner)
  /config@abc123.json  ← Conflict copy with hash suffix
```

## Cache Architecture

```rust
pub struct FileCache {
    /// Content cache: path → file bytes
    content: Cache<String, CachedContent>,
    /// Attribute cache: path → size, is_dir, mtime
    attrs: Cache<String, CachedAttr>,
    /// Directory listing cache: path → entries
    dirs: Cache<String, Vec<CachedDirEntry>>,
}
```

- **Backend**: `moka` (concurrent LRU cache)
- **TTL**: Configurable per-mount (default 60s)
- **Size limit**: Configurable per-mount (default 100MB)
- **Invalidation**: On local writes, on sync events

## Configuration

Mount configuration is stored in SQLite (`fuse_mounts` table):

| Column | Type | Description |
|--------|------|-------------|
| mount_id | UUID | Primary key |
| bucket_id | UUID | Which bucket to mount |
| mount_point | TEXT | Local filesystem path |
| enabled | BOOL | Is this mount enabled? |
| auto_mount | BOOL | Start on daemon startup? |
| read_only | BOOL | Prevent writes? |
| cache_size_mb | INT | Cache size limit |
| cache_ttl_secs | INT | Cache entry TTL |
| status | TEXT | stopped/starting/running/error |

## Platform Support

| Platform | FUSE Implementation | Unmount Command |
|----------|--------------------|-----------------|
| macOS | macFUSE | `umount` / `diskutil unmount force` |
| Linux | FUSE kernel module | `fusermount -u` / `fusermount -uz` |

Mount options vary by platform (see `mount_manager.rs`).
