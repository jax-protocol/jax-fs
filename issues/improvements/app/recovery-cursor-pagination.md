# Recovery: Cursor-Based Pagination for Object Storage Listing

**Status:** Planned
**Priority:** Low

## Problem

`recover_from_storage()` calls `storage.list_data_hashes()` which collects all object keys into a `Vec<String>` in memory before processing. For stores with millions of blobs this will OOM or stall before recovery even starts.

## Current Code

`crates/object-store/src/object_store.rs` (test-only `recover_from_storage`):

```rust
let hashes = self.storage.list_data_hashes().await?;  // loads all keys into memory
for hash_str in hashes { ... }
```

`crates/object-store/src/storage.rs` (`list_data_hashes`):

```rust
let items: Vec<_> = stream.try_collect().await?;  // collects entire listing
```

## Proposed Fix

1. Change `Storage::list_data_hashes` to return a `Stream<Item = Result<String>>` instead of `Vec<String>`.
2. Have `recover_from_storage` consume the stream with cursor-based pagination, processing blobs in batches.
3. Add progress logging (e.g. every 1000 blobs).

```rust
// storage.rs
pub fn list_data_hashes_stream(&self) -> impl Stream<Item = Result<String>> + '_ {
    let prefix = ObjectPath::from("data/");
    self.inner.list(Some(&prefix)).map(|r| {
        r.map(|meta| meta.location.as_ref().strip_prefix("data/").unwrap().to_string())
         .map_err(Into::into)
    })
}

// object_store.rs
async fn recover_from_storage(&self) -> Result<RecoveryStats> {
    let mut stats = RecoveryStats::default();
    let mut stream = std::pin::pin!(self.storage.list_data_hashes_stream());
    while let Some(hash_str) = stream.try_next().await? {
        stats.found += 1;
        // ... process one at a time, no full collection
    }
    Ok(stats)
}
```

## Files to Modify

| File | Changes |
|------|---------|
| `crates/object-store/src/storage.rs` | Add streaming `list_data_hashes_stream` method |
| `crates/object-store/src/object_store.rs` | Update `recover_from_storage` to consume stream |

## Acceptance Criteria

- [ ] Recovery does not load all keys into memory at once
- [ ] Progress logged periodically during recovery
- [ ] Existing tests still pass
- [ ] `cargo clippy` clean
