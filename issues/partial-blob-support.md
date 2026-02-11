# Partial Blob Support

**Status:** Planned
**Follow-up from:** Gateway-Tauri-Split ticket 1 (SQLite blob store)

## Objective

Add partial blob tracking and BAO tree traversal to the SQLite + Object Storage blob store, enabling resume-from-object-storage and correct `export_bao` for verified streaming.

## Background

The `jax-object-store` crate stores blobs in SQLite (metadata) + S3/local (data). Currently:
- Blobs are assumed fully present — no partial state tracking
- `export_bao` uses a simplified implementation that doesn't do full BAO tree traversal

## Implementation Steps

### 1. Track partial blob state

**File:** `crates/object-store/src/object_store.rs`

- Add `partial` column or state to blob metadata
- Track which ranges of a blob have been stored
- Allow `import_blob` to resume from where it left off

### 2. Resume from object storage

- On partial blob detection, fetch remaining ranges from S3
- Combine local and remote ranges to reconstruct full blob
- Mark blob as complete once all ranges are present

### 3. Full BAO tree traversal for export_bao

**File:** `crates/object-store/src/object_store.rs`

- Implement proper BAO tree traversal using `bao-tree` crate
- Generate correct outboard data for verified streaming
- Replace current simplified implementation

## Acceptance Criteria

- [ ] Partial blobs tracked in SQLite metadata
- [ ] Interrupted imports can resume
- [ ] `export_bao` produces correct outboard data
- [ ] Verified streaming works end-to-end
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings
