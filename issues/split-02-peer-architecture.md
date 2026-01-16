# Peer Architecture: Gateway vs Local

**Status:** Planned
**Epic:** [binary-split.md](./binary-split.md)
**Dependencies:** [public-01-api-endpoints.md](./public-01-api-endpoints.md)

## Objective

Define and implement the architectural split between gateway (mirror peer) and local (owner peer) while maintaining a consistent underlying peer implementation.

## Core Principle

**Both binaries share the same peer implementation.** The difference is in:
1. What buckets they can access
2. What operations they can perform
3. What HTTP endpoints they expose

## Shared Infrastructure

Both gateway and local use identical:

```
┌─────────────────────────────────────────────┐
│              Peer Implementation            │
├─────────────────────────────────────────────┤
│  SQLite Database (BucketLogProvider)        │
│  - Tracks bucket heads                      │
│  - Stores sync state                        │
│  - Same schema for both                     │
├─────────────────────────────────────────────┤
│  Filesystem BlobStore                       │
│  - Stores encrypted content                 │
│  - Same format for both                     │
│  - Content-addressed by hash                │
├─────────────────────────────────────────────┤
│  P2P Sync Protocol                          │
│  - iroh-based networking                    │
│  - Bucket state replication                 │
│  - Blob transfer                            │
└─────────────────────────────────────────────┘
```

## Architecture Differences

| Aspect | Gateway (Mirror) | Local (Owner) |
|--------|------------------|---------------|
| **Peer Role** | Mirror | Owner |
| **Bucket Access** | Published buckets only | All owned buckets |
| **Can Decrypt** | Only if published | Always (owns secret) |
| **Write Operations** | None | Full CRUD |
| **HTTP Endpoints** | Gateway routes only | Full API + HTML UI |
| **Sync Direction** | Pull from owners | Push/pull with peers |

## Gateway Binary

```
jax-gateway
├── Initialization
│   └── Creates peer as Mirror role
├── Bucket Filtering
│   └── Only serves published buckets
├── HTTP Routes
│   ├── GET /gw/:bucket_id/*path  (gateway content)
│   └── GET /_status/*            (health checks)
└── No Write API
```

**Key Behavior:**
- On startup, syncs published buckets from configured owner peers
- Filters bucket list to only show `is_published == true`
- Cannot mount unpublished buckets (MirrorCannotMount error)
- Read-only: no add, update, delete, mkdir, mv operations

## Local Binary

```
jax-local
├── Initialization
│   └── Creates peer as Owner role
├── Bucket Access
│   └── All owned buckets accessible
├── HTTP Routes
│   ├── HTML UI (port 8080)
│   │   └── Bucket explorer, file browser
│   ├── API (port 3000)
│   │   ├── Bucket CRUD
│   │   ├── File operations
│   │   ├── Mirror management
│   │   └── Publish control
│   └── Gateway routes (optional)
└── Full Write Access
```

**Key Behavior:**
- Full owner access to all buckets
- Can add/remove mirrors
- Can publish buckets to mirrors
- Syncs with other owner peers bidirectionally

## Implementation Plan

### 1. Gateway Bucket Filtering

The gateway must filter buckets to only serve published ones:

```rust
// In gateway handler
async fn serve_bucket(bucket_id: Uuid, state: &ServiceState) -> Result<...> {
    let mount = state.peer().mount(bucket_id).await?;

    // Check if bucket is published
    if !mount.is_published().await {
        return Err(GatewayError::NotPublished(bucket_id));
    }

    // Serve content...
}
```

### 2. Peer Initialization

Both binaries use the same `ServiceState::from_config()` but the config determines role:

```rust
// gateway/src/main.rs
let config = Config {
    // ... common config
    peer_role: PeerRole::Mirror,  // NEW: explicit role
};

// local/src/main.rs
let config = Config {
    // ... common config
    peer_role: PeerRole::Owner,
};
```

### 3. Sync Protocol Updates

Gateway needs to:
- Only request buckets where it's listed as a mirror
- Handle "not published" gracefully during sync
- Re-sync when a bucket becomes published

## Files to Modify

| File | Changes |
|------|---------|
| `service/src/config.rs` | Add `peer_role` field |
| `service/src/state.rs` | Use role during peer init |
| `gateway/src/main.rs` | Set Mirror role |
| `local/src/main.rs` | Set Owner role |
| `service/src/http/gateway.rs` | Add published check |

## Testing Strategy

1. **Local creates bucket, adds gateway as mirror**
   ```bash
   # On local
   curl -X POST localhost:3000/api/v0/bucket -d '{"name":"test"}'
   curl -X POST localhost:3000/api/v0/bucket/$ID/mirror \
     -d '{"public_key":"GATEWAY_PUBKEY"}'
   ```

2. **Gateway cannot access unpublished bucket**
   ```bash
   # On gateway
   curl localhost:8080/gw/$BUCKET_ID/
   # Expected: 404 or "not published" error
   ```

3. **Local publishes bucket**
   ```bash
   curl -X POST localhost:3000/api/v0/bucket/$ID/publish
   ```

4. **Gateway syncs and can now serve**
   ```bash
   # Wait for sync...
   curl localhost:8080/gw/$BUCKET_ID/
   # Expected: bucket content
   ```

## Acceptance Criteria

- [ ] Gateway only serves published buckets
- [ ] Gateway returns clear error for unpublished buckets
- [ ] Local can publish buckets to mirrors
- [ ] Sync protocol respects publish status
- [ ] Both binaries use same SQLite schema
- [ ] Both binaries use same blob storage format
- [ ] Config explicitly sets peer role

## Current State

**Already implemented:**
- [x] Gateway and local binaries exist
- [x] Service crate with shared infrastructure
- [x] Mirror/publish API endpoints
- [x] Mount.is_published() check
- [x] MirrorCannotMount error

**Still needed:**
- [ ] Gateway published-only filtering
- [ ] Explicit peer role in config
- [ ] Sync protocol awareness of publish status
