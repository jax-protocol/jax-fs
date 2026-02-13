# Conflict Resolution Not Wired Into Sync or FUSE

**Status:** Critical Gap
**Affects:** Data integrity during concurrent edits
**Related:** `agents/concepts/conflict-resolution.md`

## Executive Summary

JaxBucket has a well-designed, well-tested conflict resolution system (`ConflictResolver` trait, `PathOpLog.merge_with_resolver()`, `Mount.merge_from()`). However, **this system is not actually used anywhere in the real code paths**. The sync engine, FUSE mounts, and UI all bypass conflict resolution entirely, leading to potential data loss during concurrent edits.

## What Exists (The Good)

### Conflict Resolution API

Located in `crates/common/src/mount/conflict/`:

```rust
pub trait ConflictResolver: Send + Sync {
    fn resolve(&self, conflict: &Conflict, local_peer: &PublicKey) -> Resolution;
}

pub enum Resolution {
    UseBase,                      // Keep local, discard incoming
    UseIncoming,                  // Replace with incoming
    KeepBoth,                     // Add both (fork)
    SkipBoth,                     // Discard both
    RenameIncoming { new_path },  // Create conflict file
}
```

### Built-in Strategies

| Strategy | Behavior | Use Case |
|----------|----------|----------|
| `LastWriteWins` | Higher timestamp wins | Default CRDT |
| `BaseWins` | Local always wins | Stable deployments |
| `ForkOnConflict` | Keep both for manual resolution | Audit workflows |
| `ConflictFile` | Rename to `file@hash` | Collaborative editing |

### PathOpLog Merge

```rust
impl PathOpLog {
    pub fn merge_with_resolver(
        &mut self,
        other: &PathOpLog,
        resolver: &dyn ConflictResolver,
        local_peer: &PublicKey,
    ) -> MergeResult { ... }
}
```

### Mount-Level Merge

```rust
impl Mount {
    pub async fn merge_from<R: ConflictResolver>(
        &mut self,
        incoming: &Mount,
        resolver: &R,
        blobs: &BlobsStore,
    ) -> Result<(MergeResult, Link), MountError> {
        // 1. Find common ancestor
        // 2. Collect ops since ancestor from both chains
        // 3. Merge with resolver
        // 4. Apply resolved state to entry tree
        // 5. Save
    }
}
```

### Test Coverage

`crates/common/tests/conflict_resolution.rs` has comprehensive tests:
- Single file conflicts
- Multi-version divergence (3+ save cycles)
- Various resolver strategies
- `merge_from()` API

**All tests pass.** The API works correctly in isolation.

---

## What's Missing (The Problem)

### Gap 1: Sync Engine Does Not Merge

**File:** `crates/common/src/peer/sync/sync_bucket.rs`

Current flow:
```
Peer A announces new manifest
    ↓
download_manifest_chain() - downloads manifest blobs
    ↓
apply_manifest_chain() - appends to bucket log
    ↓
Done. No merge.
```

The `apply_manifest_chain` function:

```rust
async fn apply_manifest_chain<L>(
    peer: &Peer<L>,
    bucket_id: Uuid,
    manifests: &[(Manifest, Link)],
) -> Result<()> {
    // Just appends to log - NO MERGE
    peer.logs().append(
        bucket_id,
        manifest.name().to_string(),
        link.clone(),
        previous,
        height,
        is_published,
    ).await?;

    // Dispatches pin download - NO MERGE
    peer.dispatch(SyncJob::DownloadPins(...)).await
}
```

**Problem:** If the local peer has unpublished changes in a live mount, they are completely ignored. The incoming manifest becomes HEAD, and local changes are orphaned.

**What should happen:**
```
Incoming manifest arrives
    ↓
Sync checks: peer.get_active_mount(bucket_id)?
    ↓
YES, mount exists → check if ops_log is dirty
    ↓
    DIRTY → merge_from(incoming, resolver) into active mount
    CLEAN → just append (current behavior)
    ↓
Append manifest to log
    ↓
Emit BucketUpdated event → consumers just reload/invalidate
```

The merge happens in sync, not in consumers. This ensures:
- One code path for conflict resolution
- FUSE/UI/etc don't duplicate merge logic
- Consistent behavior across all mount types

### Gap 2: FUSE Mount Manager Replaces Instead of Merging

**File:** `crates/daemon/src/fuse/mount_manager.rs`

Current `on_bucket_synced()`:

```rust
pub async fn on_bucket_synced(&self, bucket_id: Uuid) -> Result<(), MountError> {
    let mounts = self.mounts.read().await;

    for (mount_id, live_mount) in mounts.iter() {
        if *live_mount.config.bucket_id == bucket_id {
            // PROBLEM: Loads fresh mount, REPLACES live mount
            let new_mount = self.peer.mount(bucket_id).await?;
            *live_mount.mount.write().await = new_mount;  // ← Local changes LOST

            live_mount.cache.invalidate_all();
        }
    }
    Ok(())
}
```

**Scenario where data is lost:**

```
1. User creates /notes.txt via FUSE
2. Mount.add() records op in live mount's ops_log
3. User hasn't flushed / no auto-save yet
4. Remote peer syncs, triggers on_bucket_synced()
5. Live mount REPLACED with remote version
6. User's /notes.txt operation is GONE
7. User sees empty directory, file vanished
```

**What should happen:**

With the active mount registry architecture, FUSE doesn't need merge logic. The sync engine already merged (if needed) before emitting the event:

```rust
pub async fn on_bucket_synced(&self, bucket_id: Uuid) -> Result<(), MountError> {
    let mounts = self.mounts.read().await;

    for (mount_id, live_mount) in mounts.iter() {
        if *live_mount.config.bucket_id == bucket_id {
            // Sync already merged if needed - just invalidate cache
            live_mount.cache.invalidate_all();

            let _ = self.sync_tx.send(SyncEvent::MountInvalidated {
                mount_id: *mount_id,
            });
        }
    }

    Ok(())
}
```

The key changes are in `start()` and `stop()`:

```rust
pub async fn start(&self, mount_id: &Uuid) -> Result<(), MountError> {
    // ... setup code ...

    // Register with Peer so sync merges into this mount
    self.peer.register_active_mount(bucket_id, mount_arc.clone());

    // ... spawn FUSE session ...
}

pub async fn stop(&self, mount_id: &Uuid) -> Result<(), MountError> {
    // Unregister before stopping
    self.peer.unregister_active_mount(bucket_id);

    // ... cleanup code ...
}
```

### Gap 3: UI Does Not Check for Conflicts Before Save

**Affected:** HTTP API handlers, Tauri IPC commands

Current UI save flow:
```
User edits file in UI
    ↓
User clicks Save
    ↓
API: POST /api/v0/buckets/:id/files/:path
    ↓
Daemon: peer.mount() → mount.add() → peer.save_mount()
    ↓
Done. No conflict check.
```

**Scenario where user blindly overwrites:**

```
1. Alice opens /report.txt (loads version at height=5)
2. Bob edits /report.txt, saves, syncs (now height=6)
3. Alice's daemon receives sync, updates to height=6
4. Alice finishes editing, clicks Save
5. Alice's edit applied to height=6 mount
6. Bob's changes are OVERWRITTEN
7. Neither Alice nor Bob knows a conflict occurred
```

The CRDT ops_log will have both operations, so technically no data is "lost" in the log. But:
- Alice never saw Bob's changes
- Alice's save created a new version that doesn't include Bob's edits
- No conflict file was created
- No warning was shown

**What should happen:**

```
User clicks Save
    ↓
API checks: has this path changed since user loaded it?
    ↓
YES → Return 409 Conflict with details
      UI shows: "This file was modified. View diff / Overwrite / Cancel"
    ↓
NO  → Save normally
```

Implementation requires:
1. Track "loaded_at" link when user opens file
2. On save, compare current HEAD to loaded_at
3. If different AND path was modified, surface conflict

---

## Affected Code Paths

### Code That Should Call merge_with_resolver() But Doesn't

| Location | Current Behavior | Should Do |
|----------|-----------------|-----------|
| `sync_bucket.rs:apply_manifest_chain()` | Append to log | Check for active mount, merge if dirty |
| `mount_manager.rs:on_bucket_synced()` | Replace live mount | Merge if ops_log non-empty |
| HTTP handlers for file save | Load → modify → save | Check for concurrent changes first |

### Code That Correctly Uses Conflict Resolution

| Location | Status |
|----------|--------|
| `crates/common/tests/conflict_resolution.rs` | ✅ Tests pass |
| Direct `Mount.merge_from()` calls | ✅ Works correctly |
| Direct `PathOpLog.merge_with_resolver()` calls | ✅ Works correctly |

---

## Architecture: Active Mount Registry in Peer

The merge logic belongs in **one place**: the sync engine at the Peer level. Consumers like FUSE just register their mounts and respond to events.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Peer                                           │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                    Active Mount Registry                             │   │
│  │              bucket_id → Arc<RwLock<Mount>>                          │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         Sync Engine                                  │   │
│  │                                                                      │   │
│  │  on_manifest_received(bucket_id, manifest):                          │   │
│  │      if active_mount = registry.get(bucket_id):                      │   │
│  │          if active_mount.ops_log.is_dirty():                         │   │
│  │              active_mount.merge_from(incoming, resolver)  ← MERGE    │   │
│  │      logs.append(manifest)                                           │   │
│  │      emit BucketUpdated(bucket_id)                                   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
└────────────────────────────────────┼────────────────────────────────────────┘
                                     │
                          BucketUpdated event
                                     │
              ┌──────────────────────┼──────────────────────┐
              │                      │                      │
              ▼                      ▼                      ▼
   ┌─────────────────────┐  ┌─────────────────────┐  ┌─────────────────────┐
   │  FUSE MountManager  │  │  Desktop UI         │  │  Other consumers    │
   │                     │  │                     │  │                     │
   │  on_bucket_updated: │  │  on_bucket_updated: │  │  on_bucket_updated: │
   │    invalidate_cache │  │    refresh_view     │  │    ...              │
   │    (no merge logic) │  │    (no merge logic) │  │                     │
   └─────────────────────┘  └─────────────────────┘  └─────────────────────┘
```

### Key Design Principles

1. **One merge code path** - All conflict resolution happens in sync engine
2. **Consumers are dumb** - FUSE, UI, etc. just register mounts and handle events
3. **Registry is the bridge** - Sync engine checks registry to know if merge is needed

### Peer API

```rust
impl Peer<L> {
    /// Active mounts that sync should merge into
    active_mounts: RwLock<HashMap<Uuid, Arc<RwLock<Mount>>>>,

    /// Register a mount as "active" - sync will merge incoming changes into it
    /// Call this when starting a FUSE mount or opening a long-lived editor session
    pub fn register_active_mount(&self, bucket_id: Uuid, mount: Arc<RwLock<Mount>>);

    /// Unregister when mount is no longer active
    /// Call this when stopping a FUSE mount or closing an editor
    pub fn unregister_active_mount(&self, bucket_id: Uuid);

    /// Check if a bucket has an active mount with pending changes
    pub async fn has_dirty_active_mount(&self, bucket_id: Uuid) -> bool;
}
```

### Sync Engine Changes

**File:** `crates/common/src/peer/sync/sync_bucket.rs`

```rust
async fn apply_manifest_chain<L>(
    peer: &Peer<L>,
    bucket_id: Uuid,
    manifests: &[(Manifest, Link)],
) -> Result<()> {
    // Check for active mount with local changes
    if let Some(active_mount) = peer.get_active_mount(bucket_id).await {
        let guard = active_mount.read().await;
        if !guard.inner().await.ops_log().is_empty() {
            drop(guard);

            // Load incoming mount from the new manifest
            let incoming = Mount::load(&manifests[0].1, peer.secret(), peer.blobs()).await?;

            // Merge INTO the active mount (preserves local changes)
            let resolver = ConflictFile::new(); // or configurable
            let mut active = active_mount.write().await;
            let (result, _link) = active.merge_from(&incoming, &resolver, peer.blobs()).await?;

            // Log what happened
            if result.total_conflicts() > 0 {
                tracing::info!(
                    "Merged {} conflicts for bucket {} using {:?}",
                    result.total_conflicts(),
                    bucket_id,
                    resolver
                );
            }
        }
    }

    // Append to log (this always happens)
    for (manifest, link) in manifests {
        peer.logs().append(
            bucket_id,
            manifest.name().to_string(),
            link.clone(),
            manifest.previous().clone(),
            manifest.height(),
            manifest.is_published(),
        ).await?;
    }

    // Emit event for consumers
    peer.emit_bucket_updated(bucket_id).await;

    Ok(())
}
```

### FUSE Changes (Simplified)

**File:** `crates/daemon/src/fuse/mount_manager.rs`

FUSE no longer needs merge logic - it just registers/unregisters and handles events:

```rust
impl MountManager {
    /// Start a mount - register with Peer for sync merging
    pub async fn start(&self, mount_id: &Uuid) -> Result<(), MountError> {
        // ... existing setup code ...

        let mount_arc = Arc::new(RwLock::new(bucket_mount));

        // Register with Peer so sync engine merges into this mount
        self.peer.register_active_mount(*bucket_id, mount_arc.clone());

        // ... spawn FUSE session ...
    }

    /// Stop a mount - unregister from Peer
    pub async fn stop(&self, mount_id: &Uuid) -> Result<(), MountError> {
        let bucket_id = /* get from config */;

        // Unregister - sync will no longer merge into this mount
        self.peer.unregister_active_mount(bucket_id);

        // ... existing cleanup code ...
    }

    /// Called when sync completes - just invalidate cache, no merge needed
    pub async fn on_bucket_synced(&self, bucket_id: Uuid) -> Result<(), MountError> {
        let mounts = self.mounts.read().await;

        for (mount_id, live_mount) in mounts.iter() {
            if *live_mount.config.bucket_id == bucket_id {
                // Just invalidate cache - sync already merged if needed
                live_mount.cache.invalidate_all();

                // Notify FUSE subscribers
                let _ = self.sync_tx.send(SyncEvent::MountInvalidated {
                    mount_id: *mount_id,
                });
            }
        }

        Ok(())
    }
}
```

### Why This Is Better

| Aspect | Old (FUSE-specific merge) | New (Peer-level merge) |
|--------|---------------------------|------------------------|
| Code paths | 2 (sync + FUSE) | 1 (sync only) |
| FUSE complexity | Knows about merging | Just cache invalidation |
| Future consumers | Must duplicate merge logic | Just register mount |
| Testing | Test in two places | Test in one place |
| Resolver config | Per-mount in FUSE | Centralized in Peer |

---

## Implementation Plan

### Phase 1: Active Mount Registry in Peer (Critical)

**File:** `crates/common/src/peer/peer_inner.rs`

1. Add `active_mounts: RwLock<HashMap<Uuid, Arc<RwLock<Mount>>>>` to `Peer`
2. Implement `register_active_mount()` and `unregister_active_mount()`
3. Add `get_active_mount()` for sync to check

```rust
impl Peer<L> {
    pub fn register_active_mount(&self, bucket_id: Uuid, mount: Arc<RwLock<Mount>>) {
        self.active_mounts.write().await.insert(bucket_id, mount);
    }

    pub fn unregister_active_mount(&self, bucket_id: Uuid) {
        self.active_mounts.write().await.remove(&bucket_id);
    }

    pub async fn get_active_mount(&self, bucket_id: Uuid) -> Option<Arc<RwLock<Mount>>> {
        self.active_mounts.read().await.get(&bucket_id).cloned()
    }
}
```

**Estimated:** 30-50 lines of code

### Phase 2: Wire Merge into Sync Engine

**File:** `crates/common/src/peer/sync/sync_bucket.rs`

1. Modify `apply_manifest_chain()` to check for active mount
2. If active mount has dirty ops, call `merge_from()` before appending
3. Use `ConflictFile` as default resolver
4. Log merge results for visibility

**Estimated:** 50-80 lines of code

### Phase 3: Update FUSE MountManager

**File:** `crates/daemon/src/fuse/mount_manager.rs`

1. Call `peer.register_active_mount()` in `start()`
2. Call `peer.unregister_active_mount()` in `stop()`
3. Simplify `on_bucket_synced()` - remove merge logic, just invalidate cache

**Estimated:** 20-30 lines changed (net reduction)

### Phase 4: Add Conflict Visibility API

**Files:** HTTP API, Tauri commands

1. Add `GET /api/v0/buckets/:id/conflicts` endpoint
2. Return list of recent conflicts from merge results
3. Add `POST /api/v0/buckets/:id/conflicts/:path/resolve` for manual resolution
4. Surface in desktop UI

**Estimated:** 150-200 lines of code

### Phase 5: UI Conflict Warning (Optional)

**Files:** HTTP handlers for file operations

1. Add optional `If-Match` header support (like HTTP ETags)
2. Track version when file is loaded
3. On save, return 409 if version changed
4. UI shows conflict dialog

**Estimated:** 100-150 lines of code

### Phase 6: Configurable Resolver Strategy (Optional)

1. Add resolver config to Peer or per-bucket
2. Allow setting strategy via API/CLI
3. Default to `ConflictFile`

**Estimated:** 50-80 lines of code

---

## Test Scenarios

### Scenario 1: FUSE Write During Sync

```
Setup:
  - Node A has FUSE mount for bucket X (registered with Peer)
  - Node B has same bucket X

Test:
  1. A: Create /test.txt via FUSE ("hello from A")
     → Mount.add() records op in ops_log
     → peer.active_mounts contains this mount
  2. B: Create /test.txt via CLI ("hello from B")
  3. B: Save and sync
  4. A: Receives sync
     → apply_manifest_chain() checks active_mounts
     → Finds A's mount with dirty ops_log
     → Calls merge_from() with ConflictFile resolver
     → Creates conflict file for B's version
     → Emits BucketUpdated event
  5. A's FUSE receives event
     → Just invalidates cache (no merge logic)

Expected (after fix):
  - /test.txt exists (A's version - local wins)
  - /test.txt@<hash> exists (B's version - conflict copy)
  - Both contents preserved
  - Merge happened in sync engine, not FUSE

Current (broken):
  - A's /test.txt vanishes OR is overwritten
  - No conflict copy
  - Data loss
```

### Scenario 2: UI Edit During Sync

```
Setup:
  - User A has file open in UI
  - User B edits same file

Test:
  1. A: Opens /doc.md (loads version v5)
  2. B: Edits /doc.md, saves (creates v6)
  3. Sync happens, A's daemon now has v6
  4. A: Finishes editing, clicks Save

Expected (after fix):
  - UI shows "File was modified. Overwrite / View diff / Cancel"
  - If overwrite, creates v7 with A's content
  - If cancel, user can reload and see B's changes

Current (broken):
  - A's save silently creates v7
  - B's changes (v6) are "lost" (in log but not in current state)
  - No warning shown
```

### Scenario 3: Multi-Peer Divergence

```
Setup:
  - 3 peers: A, B, C all have bucket X
  - All go offline

Test:
  1. A: Creates /config.json with {"env": "dev"}
  2. B: Creates /config.json with {"env": "prod"}
  3. C: Creates /config.json with {"env": "staging"}
  4. All come online, sync

Expected (after fix):
  - One /config.json (winner by CRDT rules)
  - Two conflict files /config.json@<hash1>, /config.json@<hash2>
  - All three versions preserved

Current (broken):
  - Depends on sync order
  - At most one conflict copy (maybe)
  - Likely data loss
```

---

## Acceptance Criteria

### Phase 1-3 (Critical - Fixes Data Loss)

- [ ] `Peer` has `active_mounts` registry with register/unregister methods
- [ ] `apply_manifest_chain()` checks for active mount before appending
- [ ] If active mount has dirty ops, `merge_from()` is called with `ConflictFile`
- [ ] FUSE `start()` calls `peer.register_active_mount()`
- [ ] FUSE `stop()` calls `peer.unregister_active_mount()`
- [ ] FUSE `on_bucket_synced()` simplified to just cache invalidation
- [ ] Conflict files (`file@hash`) created for Add vs Add conflicts
- [ ] `MergeResult` logged for visibility
- [ ] All existing tests pass
- [ ] New integration test for sync with active mount conflict scenario
- [ ] `cargo clippy` clean

### Phase 4-6 (Improvements)

- [ ] Conflicts surfaced via REST API
- [ ] UI shows conflict list with resolution options
- [ ] UI save checks for concurrent modifications (409 Conflict)
- [ ] Resolver strategy configurable per bucket
- [ ] Documentation updated (`agents/concepts/conflict-resolution.md`)

---

## References

### Files to Modify

- `crates/common/src/peer/peer_inner.rs` - Add active_mounts registry
- `crates/common/src/peer/sync/sync_bucket.rs` - Wire merge into apply_manifest_chain
- `crates/daemon/src/fuse/mount_manager.rs` - Register/unregister, simplify on_bucket_synced

### Existing Conflict Resolution Code

- `agents/concepts/conflict-resolution.md` - Conflict resolution design doc
- `crates/common/src/mount/conflict/` - Resolver implementations
- `crates/common/src/mount/path_ops.rs` - PathOpLog merge
- `crates/common/src/mount/mount_inner.rs` - Mount.merge_from()
- `crates/common/tests/conflict_resolution.rs` - Existing tests (use as reference)
