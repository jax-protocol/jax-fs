# Bucket allowlist: approval, removal, and sync filtering

**Status:** Open
**Priority:** High
**Category:** Feature
**Auto:** false

## Objective

Add a bucket status model (`pending`, `active`, `ignored`) so peers can approve incoming shares before syncing content, remove buckets locally without destroying logs, and ignore future updates from peers they don't want to sync with.

## Background

Currently, if someone adds your key to a bucket's shares, your peer automatically syncs it on the next ping cycle. There is no way to:

1. **Approve/reject** a shared bucket before content downloads
2. **Remove** a bucket locally (stop syncing, purge blobs, unmount FUSE)
3. **Ignore** future sync attempts for a bucket you don't want

The bucket_log should be preserved as an audit trail even when a bucket is removed.

## Design

### Bucket status model

New `bucket_status` table (separate from immutable `bucket_log`):

```sql
CREATE TABLE bucket_status (
    bucket_id TEXT PRIMARY KEY,
    status TEXT NOT NULL DEFAULT 'pending',  -- 'pending' | 'active' | 'ignored'
    shared_by TEXT,                          -- peer public key hex (NULL for self-created)
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

- **`pending`**: newly shared with us. Accept manifest/log entries but do NOT download pins/blobs.
- **`active`**: approved. Sync normally.
- **`ignored`**: explicitly rejected or removed. Do not sync. Optionally purge blobs.
- **No row + exists in bucket_log**: treat as `active` (backward compat for pre-migration buckets).

### Sync flow changes

Four gating points where status must be checked:

1. **`apply_manifest_chain`** (`crates/common/src/peer/sync/sync_bucket.rs:400`): gate `DownloadPinsJob` dispatch on bucket status. Still apply manifest chain (audit trail), but skip blob downloads for non-active buckets.

2. **`schedule_periodic_pings`** (`crates/daemon/src/sync_provider.rs:164`): filter `list_buckets()` to only ping for `active` buckets. Don't waste bandwidth pinging for ignored/pending buckets.

3. **Incoming ping handler** (`crates/common/src/peer/protocol/messages/ping.rs`): in `handle_message_side_effect`, check status before dispatching `SyncBucketJob`. For `NotFound` (new bucket), create a `pending` status record.

4. **New bucket detection** in `sync_bucket::execute` (line 67): when `exists` is false, signal that this is a newly discovered bucket so the daemon can set `pending` status.

### BucketLogProvider trait extension

Since sync logic lives in `crates/common` but bucket status is a daemon concern, add default methods to `BucketLogProvider` (`crates/common/src/bucket_log/provider.rs`):

```rust
/// Whether content (pins/blobs) should be synced for this bucket.
/// Implementations override to support approval workflows.
async fn should_sync_content(&self, id: Uuid) -> Result<bool, BucketLogError<Self::Error>> {
    Ok(true)
}

/// Called when a new bucket is first discovered from a remote peer.
/// Implementations can use this to set initial status (e.g. pending).
async fn on_new_bucket_discovered(&self, id: Uuid, shared_by: Option<String>)
    -> Result<(), BucketLogError<Self::Error>> {
    Ok(())
}

/// List only buckets that should be actively synced.
async fn list_syncable_buckets(&self) -> Result<Vec<Uuid>, BucketLogError<Self::Error>> {
    self.list_buckets().await
}
```

The `Database` implementation overrides these to check `bucket_status`.

### API endpoints

**`POST /api/v0/bucket/approve`** — move `pending` -> `active`
- Triggers catch-up: dispatch `DownloadPinsJob` for all existing manifest entries that were skipped while pending
- Request: `{ bucket_id: Uuid }`

**`POST /api/v0/bucket/ignore`** — move any status -> `ignored`
- Unmount any FUSE mounts for this bucket
- Stop syncing
- Optionally purge blobs (`purge_blobs: bool`)
- Preserve bucket_log entries
- Request: `{ bucket_id: Uuid, purge_blobs: Option<bool> }`

**`POST /api/v0/bucket/list`** — add status to response
- Add `status: String` field to `BucketInfo`
- Add optional `status` filter to `ListRequest`
- Left join with `bucket_status`, default to `"active"` for legacy rows

### Blob purge (on ignore with purge_blobs=true)

1. Walk all `bucket_log` entries for the bucket
2. Load each manifest, collect pins hashes
3. Delete blobs via `BlobsStore` (needs a new `delete_hash` method)
4. Accept that content-addressed blobs shared across buckets may need re-download — self-heals on next sync

## Files to modify

- `crates/common/src/bucket_log/provider.rs` — add trait default methods
- `crates/common/src/peer/sync/sync_bucket.rs` — gate `DownloadPinsJob`, call `on_new_bucket_discovered`
- `crates/daemon/src/sync_provider.rs` — use `list_syncable_buckets` in periodic pings
- `crates/common/src/peer/protocol/messages/ping.rs` — check status in side effects
- `crates/daemon/src/database/bucket_log_provider.rs` — implement trait overrides
- `crates/daemon/src/database/types/` — new `BucketStatus` type (follow `mount_status.rs` pattern)
- `crates/daemon/src/database/` — new `bucket_status_queries.rs`
- `crates/daemon/src/http_server/api/v0/bucket/` — new `approve.rs`, `ignore.rs`; update `list.rs`
- `crates/daemon/src/http_server/api/v0/bucket/create.rs` — auto-set `active` on self-created buckets
- Migration file for `bucket_status` table

## Implementation order

1. Schema + types (no behavior change): migration, `BucketStatus` enum, CRUD queries
2. Auto-set `active` on bucket create
3. `BucketLogProvider` trait extension + `Database` overrides
4. Sync flow gating (the core change)
5. API endpoints (approve, ignore, list update)
6. Blob purge (can defer to follow-up)

## Acceptance criteria

- [ ] Self-created buckets are automatically `active`
- [ ] Buckets shared by a remote peer start as `pending`
- [ ] `pending` buckets appear in list but content is not downloaded
- [ ] Approving a `pending` bucket triggers content download catch-up
- [ ] `ignored` buckets are not pinged, not synced, FUSE unmounted
- [ ] `bucket_log` entries preserved when bucket is ignored
- [ ] Pre-migration buckets (no status row) behave as `active`
- [ ] `cargo test && cargo clippy` pass
