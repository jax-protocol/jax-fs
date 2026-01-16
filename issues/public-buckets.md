# Public Buckets & Mirror Peers

**Status:** In Progress

## Background

JaxBucket currently operates with a single principal type: **Owner**. Owners have full read/write access to buckets and receive encrypted shares of the bucket's secret key. This works well for collaborative editing but doesn't support a common use case: **publicly serving bucket content** through a read-only gateway.

This epic introduces:
1. **Published buckets** - Buckets that can be served publicly via a gateway
2. **Mirror peers** - Read-only peers that can sync and serve published buckets

## Architecture

### Principal Roles

```
┌─────────────────────────────────────────────────────────────┐
│                        Bucket                                │
│                                                              │
│   Owners (full access)           Mirrors (read-only)        │
│   ┌──────────────────┐           ┌──────────────────┐       │
│   │ - Read/write     │           │ - Read only      │       │
│   │ - Encrypted share│           │ - Placeholder    │◄──┐   │
│   │ - Sync updates   │           │   share (unpub)  │   │   │
│   │ - Add principals │           │ - Real share     │───┘   │
│   └──────────────────┘           │   (published)    │       │
│                                  │ - Follow state   │       │
│                                  └──────────────────┘       │
└─────────────────────────────────────────────────────────────┘
```

### Publishing Flow

```
1. Owner creates bucket (normal flow)
2. Owner adds mirror peer: add_mirror(gateway_public_key)
   - Mirror gets placeholder share (all zeros)
   - Mirror can see bucket exists but cannot decrypt

3. Owner publishes bucket: publish()
   - Secret key encrypted to all mirror public keys
   - Mirrors receive real shares
   - Mirrors can now decrypt and serve content

4. Owner unpublishes: unpublish()
   - Mirror shares reverted to placeholders
   - Mirrors can no longer decrypt
```

### Share States

| State | Owner | Mirror (unpublished) | Mirror (published) |
|-------|-------|---------------------|-------------------|
| Has share | Yes (encrypted) | Yes (placeholder) | Yes (encrypted) |
| Can decrypt | Yes | No | Yes |
| Can write | Yes | No | No |
| Sees updates | Yes | Yes | Yes |

## Current State (What's Done)

The core primitives are implemented in `common/`:

**`common/src/mount/principal.rs`:**
```rust
pub enum PrincipalRole {
    Owner,   // Full read/write access
    Mirror,  // Read-only, can serve published buckets
}
```

**`common/src/crypto/secret_share.rs`:**
```rust
impl SecretShare {
    /// Check if this share is a placeholder (all zeros)
    pub fn is_placeholder(&self) -> bool { ... }
}
```

**`common/src/mount/manifest.rs`:**
```rust
impl Share {
    pub fn new_mirror(public_key: PublicKey) -> Self { ... }
    pub fn can_decrypt(&self) -> bool { ... }
    pub fn is_mirror(&self) -> bool { ... }
    pub fn is_owner(&self) -> bool { ... }
}

impl Manifest {
    pub fn add_mirror(&mut self, public_key: PublicKey) { ... }
    pub fn remove_mirror(&mut self, public_key: &PublicKey) -> Option<Share> { ... }
    pub fn get_mirrors(&self) -> Vec<&Share> { ... }
    pub fn get_owners(&self) -> Vec<&Share> { ... }
    pub fn is_published(&self) -> bool { ... }
    pub fn publish(&mut self, secret: Secret) -> Result<(), SecretShareError> { ... }
    pub fn unpublish(&mut self) { ... }
}
```

**`common/src/mount/mount_inner.rs`:**
```rust
impl Mount {
    pub fn add_mirror(&mut self, public_key: PublicKey) -> Result<(), MountError> { ... }
    pub fn remove_mirror(&mut self, public_key: &PublicKey) -> Result<Option<Share>, MountError> { ... }
    pub fn is_published(&self) -> bool { ... }
    pub fn publish(&mut self) -> Result<(), MountError> { ... }
    pub fn unpublish(&mut self) -> Result<(), MountError> { ... }
}
```

## What's Missing

1. **API Endpoints** - HTTP endpoints to manage mirrors and publishing
2. **Sync Protocol** - How mirrors discover and sync published buckets
3. **Gateway Access Control** - Gateway should only serve published buckets

## Tickets

| Ticket | Description | Status |
|--------|-------------|--------|
| [public-01-api-endpoints](./public-01-api-endpoints.md) | API endpoints for mirror/publish management | Planned |
| [public-02-sync-protocol](./public-02-sync-protocol.md) | Mirror sync and discovery | Planned |
| [public-03-gateway-access](./public-03-gateway-access.md) | Gateway access control for published buckets | Planned |

## Key Decisions

### Why Placeholder Shares?

When a mirror is added to an unpublished bucket, it receives a "placeholder" share (all zeros). This allows:
- The mirror to see the bucket exists in their bucket list
- The mirror to receive state update notifications
- The owner to publish/unpublish without re-adding mirrors

Without placeholders, we'd need to track mirrors separately from the share list.

### Why Encrypt to Mirrors on Publish?

When publishing, we encrypt the bucket's secret key to each mirror's public key. This is the same mechanism used for owners. Benefits:
- Reuses existing share infrastructure
- Mirrors can decrypt using standard flow
- Unpublishing just replaces shares with placeholders

### Mirror Cannot Write

Even with a valid share, mirrors cannot write because:
1. The `save()` function checks `share.is_owner()`
2. API endpoints should reject write operations from mirrors
3. Sync protocol should not accept pushes from mirrors

## Verification Checklist

- [ ] Owner can add mirror via API
- [ ] Mirror appears in bucket's share list with placeholder
- [ ] Owner can publish bucket via API
- [ ] Mirror's placeholder replaced with real share
- [ ] Mirror can decrypt and read published bucket
- [ ] Owner can unpublish bucket via API
- [ ] Mirror can no longer decrypt after unpublish
- [ ] Gateway only serves published buckets
- [ ] Mirror sync works for published buckets
