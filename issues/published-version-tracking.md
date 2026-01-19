# Published Version Tracking in Bucket Log

**Status:** Planned

## Objective

Track which bucket versions are published in the bucket log, enabling efficient lookup of the latest published version for mirrors and gateways.

## Background

Mirrors can only decrypt bucket contents after publication (when `manifest.public` is set). Currently, to find the latest published version:

1. Get all log entries
2. Load each manifest from the blob store
3. Check `manifest.is_published()` for each

This is inefficient for gateways serving mirrors. By tracking publication status in the bucket log, gateways can directly query for the latest published version.

## Implementation Steps

### 1. Add migration for published column

**File:** `crates/app/migrations/YYYYMMDDHHMMSS_add_published_to_bucket_log.up.sql`

```sql
ALTER TABLE bucket_log ADD COLUMN published BOOLEAN NOT NULL DEFAULT FALSE;

-- Index for efficient published version queries
CREATE INDEX idx_bucket_log_bucket_published
    ON bucket_log(bucket_id, published, height DESC);
```

**File:** `crates/app/migrations/YYYYMMDDHHMMSS_add_published_to_bucket_log.down.sql`

```sql
DROP INDEX idx_bucket_log_bucket_published;
ALTER TABLE bucket_log DROP COLUMN published;
```

### 2. Extend BucketLogProvider trait

**File:** `crates/common/src/bucket_log/provider.rs`

```rust
/// Append a version of the bucket to the log
async fn append(
    &self,
    id: Uuid,
    name: String,
    current: Link,
    previous: Option<Link>,
    height: u64,
    published: bool,  // NEW PARAMETER
) -> Result<(), BucketLogError<Self::Error>>;

/// Get the latest published version of a bucket
///
/// Returns the highest-height entry where published = true
async fn latest_published(
    &self,
    id: Uuid,
) -> Result<Option<(Link, u64)>, BucketLogError<Self::Error>>;
```

### 3. Update MemoryBucketLogProvider

**File:** `crates/common/src/bucket_log/memory.rs`

Add `published: bool` field to the in-memory entry struct and implement `latest_published()`.

### 4. Update SqliteBucketLogProvider

**File:** `crates/app/src/daemon/database/bucket_log_provider.rs`

Update append query to include published column:

```rust
sqlx::query!(
    r#"
    INSERT INTO bucket_log (bucket_id, name, current_link, previous_link, height, published)
    VALUES (?, ?, ?, ?, ?, ?)
    "#,
    bucket_id, name, current, previous, height, published
)
```

Add `latest_published` implementation:

```rust
async fn latest_published(&self, id: Uuid) -> Result<Option<(Link, u64)>, ...> {
    let row = sqlx::query!(
        r#"
        SELECT current_link, height
        FROM bucket_log
        WHERE bucket_id = ? AND published = TRUE
        ORDER BY height DESC
        LIMIT 1
        "#,
        id.to_string()
    )
    .fetch_optional(&self.pool)
    .await?;

    Ok(row.map(|r| (Link::from_str(&r.current_link).unwrap(), r.height as u64)))
}
```

### 5. Update save_mount to pass publication status

**File:** `crates/common/src/peer/peer_inner.rs`

When calling `bucket_log.append()`, check if manifest is published:

```rust
self.bucket_log
    .append(
        id,
        name,
        current_link,
        previous_link,
        height,
        manifest.is_published(),  // Pass publication status
    )
    .await?;
```

### 6. Add API endpoint for latest published

**File:** `crates/app/src/daemon/http_server/api/v0/bucket/latest_published.rs`

```rust
#[derive(Serialize)]
pub struct LatestPublishedResponse {
    pub bucket_id: Uuid,
    pub link: Option<String>,
    pub height: Option<u64>,
}

pub async fn handler(
    State(state): State<ServiceState>,
    Path(bucket_id): Path<Uuid>,
) -> Result<Json<LatestPublishedResponse>, ...> {
    let result = state.peer().bucket_log().latest_published(bucket_id).await?;
    Ok(Json(LatestPublishedResponse {
        bucket_id,
        link: result.map(|(l, _)| l.to_string()),
        height: result.map(|(_, h)| h),
    }))
}
```

## Files to Modify/Create

| File | Changes |
|------|---------|
| `crates/app/migrations/...` | Add `published` column migration |
| `crates/common/src/bucket_log/provider.rs` | Add `published` param to append, add `latest_published()` |
| `crates/common/src/bucket_log/memory.rs` | Update for published tracking |
| `crates/app/src/daemon/database/bucket_log_provider.rs` | Update queries for published |
| `crates/common/src/peer/peer_inner.rs` | Pass `is_published()` to append |
| `crates/app/src/daemon/http_server/api/v0/bucket/latest_published.rs` | New endpoint |
| `crates/app/src/daemon/http_server/api/v0/bucket/mod.rs` | Register new endpoint |

## Acceptance Criteria

- [ ] `bucket_log` table has `published` column
- [ ] `append()` accepts and stores publication status
- [ ] `latest_published()` returns most recent published version
- [ ] Gateway can efficiently serve mirrors the latest published content
- [ ] Existing functionality unaffected (migration handles defaults)
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings

## Verification

```bash
# Create and publish a bucket
jax-bucket bucket create --name test
jax-bucket bucket publish --bucket-id <id>

# Query latest published via API
curl http://localhost:3000/api/v0/bucket/<id>/latest-published
# Should return the published version link and height

# Make unpublished changes
jax-bucket file add --bucket-id <id> --path /new-file.txt --content "..."

# Query again - should still return the previous published version
curl http://localhost:3000/api/v0/bucket/<id>/latest-published
```
