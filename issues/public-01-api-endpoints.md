# API Endpoints for Mirror/Publish Management

**Status:** Complete
**Epic:** [public-buckets.md](./public-buckets.md)
**Dependencies:** None

## Objective

Add HTTP API endpoints to the local peer for managing mirrors and publishing buckets.

## Endpoints

### Add Mirror

```
POST /api/v0/bucket/:bucket_id/mirror
Content-Type: application/json

{
  "public_key": "base64-or-hex-encoded-public-key"
}
```

Response:
```json
{
  "bucket_id": "uuid",
  "mirror_added": "public-key",
  "total_mirrors": 2
}
```

### Remove Mirror

```
DELETE /api/v0/bucket/:bucket_id/mirror/:public_key
```

Response:
```json
{
  "bucket_id": "uuid",
  "mirror_removed": "public-key",
  "total_mirrors": 1
}
```

### Publish Bucket

```
POST /api/v0/bucket/:bucket_id/publish
```

Response:
```json
{
  "bucket_id": "uuid",
  "published": true,
  "mirrors_with_access": 2
}
```

## Implementation

### Files Created

- `crates/local/src/http/api/v0/bucket/mirror.rs` - Mirror add/remove handlers
- `crates/local/src/http/api/v0/bucket/publish.rs` - Publish handler

### Files Modified

- `crates/local/src/http/api/v0/bucket/mod.rs` - Added new routes

## Acceptance Criteria

- [x] `POST /api/v0/bucket/:id/mirror` adds a mirror with placeholder share
- [x] `DELETE /api/v0/bucket/:id/mirror/:key` removes a mirror
- [x] `POST /api/v0/bucket/:id/publish` encrypts secret to all mirrors
- [x] Invalid public key returns 400 Bad Request
- [x] Mirror not found returns 404

## Verification

```bash
# Start local peer
./target/release/jax-local --database ./test.db --blobs ./blobs

# Create a bucket
curl -X POST http://localhost:3000/api/v0/bucket \
  -H "Content-Type: application/json" \
  -d '{"name": "test-bucket"}'

# Add a mirror (use a real gateway public key)
curl -X POST http://localhost:3000/api/v0/bucket/$BUCKET_ID/mirror \
  -H "Content-Type: application/json" \
  -d '{"public_key": "BASE64_GATEWAY_PUBKEY"}'

# Publish bucket
curl -X POST http://localhost:3000/api/v0/bucket/$BUCKET_ID/publish
```
