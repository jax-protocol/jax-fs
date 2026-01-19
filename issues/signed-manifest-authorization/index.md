# Signed Manifest Authorization

## Background

The current Providence tracking system has a security flaw: it does not verify that bucket updates come from authorized peers with the correct permissions.

**Current behavior:**
- `verify_provenance()` only checks if the *receiving* peer is in the manifest's shares
- No verification of *who created* the update
- Any peer could potentially broadcast fake updates to a bucket
- Mirrors (read-only role) could inject state changes
- Share modifications and publishing are not verified against the author's role

**Security impact:**
- Malicious or compromised peers could inject unauthorized updates
- Read-only mirrors could escalate to write operations
- No cryptographic proof of authorship for state changes

## Solution

Sign each manifest with the author's Ed25519 key and validate during sync:

1. Add `author` (public key) and `signature` fields to `Manifest`
2. Sign the manifest content (excluding signature) when saving
3. During sync, verify:
   - Signature is valid for the manifest content
   - Author public key is in the manifest's shares
   - Author has `Owner` role (only owners can create updates)
4. Reject updates that fail validation

## Architecture

```
Current (insecure):
  Owner saves → Manifest (unsigned) → Peer receives → verify_provenance(our key in shares?) → Accept

Proposed (secure):
  Owner saves → Manifest (signed by author) → Peer receives → Validate:
    1. Signature valid?
    2. Author in shares?
    3. Author role == Owner?
  → Accept or Reject
```

## Tickets

| # | Ticket | Status | Description |
|---|--------|--------|-------------|
| 0 | [Manifest signature](./0-manifest-signature.md) | Done | Add author/signature fields to Manifest |
| 1 | [Sync validation](./1-sync-validation.md) | Planned | Validate signature and author during sync |
| 2 | [Role enforcement](./2-role-enforcement.md) | Planned | Reject updates from non-owners |

## Execution Order

**Stage 1:** Ticket 0 - Add signature support (backwards-compatible, optional signature)
**Stage 2:** Ticket 1 - Validate signatures during sync
**Stage 3:** Ticket 2 - Enforce role-based permissions

## Key Files

| File | Role |
|------|------|
| `crates/common/src/mount/manifest.rs` | Manifest structure (add author, signature) |
| `crates/common/src/mount/mount_inner.rs` | Mount::save() (sign manifest) |
| `crates/common/src/peer/sync/sync_bucket.rs` | Sync validation (verify signature + role) |
| `crates/common/src/mount/principal.rs` | PrincipalRole definitions |
