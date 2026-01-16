# Gateway Access Control for Published Buckets

**Status:** Planned
**Epic:** [public-buckets.md](./public-buckets.md)
**Dependencies:** [public-01-api-endpoints](./public-01-api-endpoints.md), [public-02-sync-protocol](./public-02-sync-protocol.md)

## Objective

Ensure the gateway only serves content from published buckets, preventing access to unpublished or private content.

## Current Behavior

The gateway handler in `service/src/http/html/gateway/mod.rs` currently:
1. Loads the mount for any bucket ID
2. Serves content if the mount loads successfully
3. Returns 503 if the bucket is still syncing

## Desired Behavior

1. Check if the bucket is published before serving
2. Return 403 Forbidden for unpublished buckets
3. Return 404 for buckets the gateway doesn't know about
4. Clear indication that content is from a published source

## Implementation Steps

1. **Add published check to gateway handler**
   ```rust
   // In gateway handler
   let mount = state.peer().mount(bucket_id).await?;

   if !mount.is_published() {
       return forbidden_response("Bucket is not published");
   }
   ```

2. **Consider caching publish status**
   - Avoid loading full mount just to check publish status
   - Could cache in database or memory

3. **Add header for published content**
   ```
   X-Jax-Published: true
   X-Jax-Publisher: <owner-public-key>
   ```

## Files to Modify

- `crates/service/src/http/html/gateway/mod.rs` - Add publish check
- `crates/gateway/src/main.rs` - Optional: Add middleware for access control

## Acceptance Criteria

- [ ] Gateway returns 403 for unpublished buckets
- [ ] Gateway returns 404 for unknown buckets
- [ ] Gateway serves content for published buckets
- [ ] Response includes `X-Jax-Published` header
- [ ] Unpublishing immediately blocks access

## Verification

```bash
# Setup: Local peer with bucket, gateway as mirror

# Before publishing
curl http://gateway:8080/gw/$BUCKET_ID/file.txt
# Expected: 403 Forbidden

# After publishing
curl http://gateway:8080/gw/$BUCKET_ID/file.txt
# Expected: 200 OK with content

# After unpublishing
curl http://gateway:8080/gw/$BUCKET_ID/file.txt
# Expected: 403 Forbidden
```
