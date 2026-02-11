# Share Role Selection

**Status:** Planned

## Objective

Add role selection (Owner or Mirror) to the share UI, allowing users to specify whether to share a bucket with a peer as a full owner or as a read-only mirror.

## Background

The system supports two roles:
- **Owner**: Full read/write access, can modify bucket contents, add/remove principals, publish
- **Mirror**: Read-only access after publication, can sync and serve content, cannot modify

Currently, the share flow only supports sharing as Owner. The `add_mirror()` method exists in the backend but is not exposed through the UI or API.

## Implementation Steps

### 1. Update ShareRequest struct

**File:** `crates/daemon/src/daemon/http_server/api/v0/bucket/share.rs`

Add `role` field with default value "owner":

```rust
#[derive(Debug, Clone, Serialize, Deserialize, clap::Args)]
pub struct ShareRequest {
    #[arg(long)]
    pub bucket_id: Uuid,

    #[arg(long)]
    pub peer_public_key: String,

    /// Role to assign: "owner" or "mirror" (defaults to "owner")
    #[arg(long, default_value = "owner")]
    pub role: String,
}
```

### 2. Update API handler

**File:** `crates/daemon/src/daemon/http_server/api/v0/bucket/share.rs`

```rust
match req.role.to_lowercase().as_str() {
    "owner" => mount.add_owner(peer_public_key).await?,
    "mirror" => mount.add_mirror(peer_public_key).await,
    _ => return Err(ShareError::InvalidRole(req.role)),
}
```

Add error variant:

```rust
#[error("Invalid role: {0}. Must be 'owner' or 'mirror'")]
InvalidRole(String),
```

### 3. Add role selector to SolidJS share UI

**Target:** Tauri desktop app (`crates/desktop/src/`)

Add Owner/Mirror radio buttons to the share dialog. The share dialog should include a role selector matching the app's grayscale design with green/red accents.

### 4. Update IPC command

**File:** `crates/desktop/src-tauri/src/commands/bucket.rs`

Include `role` parameter in the `share` IPC command, forwarded to the REST API.

### 5. Update TypeScript API client

**File:** `crates/desktop/src/lib/api.ts`

Add `role` field to the share request type.

## Files to Modify

| File | Changes |
|------|---------|
| `crates/daemon/src/daemon/http_server/api/v0/bucket/share.rs` | Add `role` field, handle owner/mirror, add error |
| `crates/desktop/src-tauri/src/commands/bucket.rs` | Add `role` param to share IPC command |
| `crates/desktop/src/lib/api.ts` | Add `role` to share request type |
| `crates/desktop/src/` (share dialog component) | Add role radio buttons |

## Acceptance Criteria

- [ ] Share dialog displays Owner/Mirror radio buttons
- [ ] Owner is selected by default
- [ ] Mirror role adds peer with `PrincipalRole::Mirror`
- [ ] API returns error for invalid role values
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings

## Verification

1. Open bucket in desktop app, click Share
2. Verify role selector with Owner default
3. Select Mirror, enter peer key, share
4. Verify peer added as Mirror via CLI or manifest inspection
