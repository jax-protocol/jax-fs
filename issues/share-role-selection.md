# Share Role Selection

**Status:** Planned

## Objective

Add role selection (Owner or Mirror) to the share modal UI, allowing users to specify whether to share a bucket with a peer as a full owner or as a read-only mirror.

## Background

The system supports two roles:
- **Owner**: Full read/write access, can modify bucket contents, add/remove principals, publish
- **Mirror**: Read-only access after publication, can sync and serve content, cannot modify

Currently, the share modal only supports sharing as Owner. The `add_mirror()` method exists in the backend but is not exposed through the UI or API.

## Implementation Steps

### 1. Update ShareRequest struct

**File:** `crates/app/src/daemon/http_server/api/v0/bucket/share.rs`

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

**File:** `crates/app/src/daemon/http_server/api/v0/bucket/share.rs`

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

### 3. Add role selector to share modal

**File:** `crates/app/templates/components/modals/share.html`

Add radio buttons after the peer public key input:

```html
<div class="share-form-field">
    <label class="share-label">
        <i class="fas fa-user-shield"></i> Role
    </label>
    <div class="share-role-options">
        <label class="share-role-option">
            <input type="radio" name="role" value="owner" checked>
            <span class="share-role-label">
                <i class="fas fa-crown"></i>
                <span class="share-role-title">Owner</span>
                <span class="share-role-desc">Full read/write access. Can modify files and manage sharing.</span>
            </span>
        </label>
        <label class="share-role-option">
            <input type="radio" name="role" value="mirror">
            <span class="share-role-label">
                <i class="fas fa-satellite-dish"></i>
                <span class="share-role-title">Mirror</span>
                <span class="share-role-desc">Read-only access after publish. Ideal for CDN/gateway nodes.</span>
            </span>
        </label>
    </div>
</div>
```

### 4. Update JavaScript

Include role in fetch request:

```javascript
const role = form.querySelector('input[name="role"]:checked').value;
body: JSON.stringify({
    bucket_id: bucketId,
    peer_public_key: peerPublicKey,
    role: role
})
```

### 5. Add CSS styles

Style the role selector to match existing design.

## Files to Modify

| File | Changes |
|------|---------|
| `crates/app/src/daemon/http_server/api/v0/bucket/share.rs` | Add `role` field, handle owner/mirror, add error |
| `crates/app/templates/components/modals/share.html` | Add role radio buttons, update JS, add CSS |

## Acceptance Criteria

- [ ] Share modal displays Owner/Mirror radio buttons
- [ ] Owner is selected by default
- [ ] Mirror role adds peer with `PrincipalRole::Mirror`
- [ ] API returns error for invalid role values
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings

## Verification

1. Open bucket in web UI, click Share
2. Verify role selector with Owner default
3. Select Mirror, enter peer key, share
4. Verify peer added as Mirror via CLI or manifest inspection
