# Mirror Sync and Discovery

**Status:** Planned
**Epic:** [public-buckets.md](./public-buckets.md)
**Dependencies:** [public-01-api-endpoints](./public-01-api-endpoints.md)

## Objective

Enable mirrors to discover and sync published buckets from owners.

## Background

Currently, peers sync buckets by:
1. Being added to a bucket's share list
2. Receiving the bucket ID through an out-of-band mechanism
3. Using the sync protocol to fetch state

For mirrors, we need to:
1. Handle the case where a mirror has a placeholder share (can see bucket, can't decrypt)
2. Transition to full access when published
3. Keep following state updates after unpublish

## Implementation Steps

1. **Update mount loading for mirrors**
   - Currently `Mount::load()` returns `MirrorCannotMount` for mirrors with placeholder
   - Should succeed for mirrors with real shares (published)
   - Should provide a "metadata-only" view for unpublished mirrors

2. **Add sync notification for publish events**
   - When owner publishes, notify mirrors via sync protocol
   - Mirrors should re-attempt mount after receiving new share

3. **Gateway discovery mechanism**
   - Gateway needs a way to discover which buckets to sync
   - Option A: Explicit subscription via config
   - Option B: Accept incoming share notifications
   - Option C: Poll known owners for published buckets

## Files to Modify

- `crates/common/src/mount/mount_inner.rs` - Update load() for mirror cases
- `crates/common/src/peer/sync.rs` - Add publish notification
- `crates/service/src/sync_provider.rs` - Handle publish events

## Acceptance Criteria

- [ ] Mirror can see bucket metadata when unpublished
- [ ] Mirror receives notification when bucket is published
- [ ] Mirror can decrypt and serve content after publish
- [ ] Mirror continues to follow state updates
- [ ] Gateway can subscribe to buckets from specific owners

## Verification

1. Start local peer with bucket
2. Start gateway peer
3. Add gateway as mirror to bucket
4. Verify gateway sees bucket but can't read content
5. Publish bucket from local
6. Verify gateway can now read and serve content
