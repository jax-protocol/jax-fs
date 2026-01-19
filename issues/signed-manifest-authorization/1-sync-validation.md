# Sync Validation

**Status:** Planned
**Depends on:** Ticket 0 (Manifest signature)

## Objective

During bucket sync, validate that incoming manifests have valid signatures and that the author is an authorized peer in the bucket's shares.

## Background

Currently, `verify_provenance()` in `sync_bucket.rs` only checks if *our* key is in the manifest's shares. It does not verify:
1. Whether the manifest has a valid signature
2. Whether the author is in the shares
3. Whether the author has write permissions

This ticket adds signature and author validation during sync.

## Implementation Steps

1. **Update `verify_provenance()` to validate signatures** (`crates/common/src/peer/sync/sync_bucket.rs`)

   Replace the current minimal check with comprehensive validation:

   ```rust
   fn verify_provenance<L>(peer: &Peer<L>, manifest: &Manifest) -> Result<ProvenanceResult> {
       // 1. Check we're authorized to receive this bucket
       let our_pub_key = PublicKey::from(*peer.secret().public());
       let our_key_hex = our_pub_key.to_hex();

       let we_are_authorized = manifest
           .shares()
           .iter()
           .any(|(key_hex, _)| key_hex == &our_key_hex);

       if !we_are_authorized {
           return Ok(ProvenanceResult::NotAuthorized);
       }

       // 2. Check manifest is signed (for new manifests)
       if !manifest.is_signed() {
           // Allow unsigned for backwards compatibility during migration
           // TODO: Make this an error after migration period
           tracing::warn!("Received unsigned manifest for bucket {}", manifest.id());
           return Ok(ProvenanceResult::UnsignedLegacy);
       }

       // 3. Verify signature
       if !manifest.verify_signature()? {
           return Ok(ProvenanceResult::InvalidSignature);
       }

       // 4. Check author is in shares
       let author = manifest.author().expect("is_signed() was true");
       let author_hex = author.to_hex();

       let author_share = manifest.shares().get(&author_hex);
       if author_share.is_none() {
           return Ok(ProvenanceResult::AuthorNotInShares);
       }

       Ok(ProvenanceResult::Valid)
   }

   enum ProvenanceResult {
       Valid,
       NotAuthorized,
       UnsignedLegacy,  // Allowed during migration
       InvalidSignature,
       AuthorNotInShares,
   }
   ```

2. **Handle validation results in sync execution**

   In the `execute()` function, reject invalid manifests:

   ```rust
   match verify_provenance(peer, &manifest)? {
       ProvenanceResult::Valid => {
           // Continue with sync
       }
       ProvenanceResult::UnsignedLegacy => {
           // Warn but allow during migration
           tracing::warn!("Accepting unsigned manifest (migration mode)");
       }
       ProvenanceResult::NotAuthorized => {
           return Err(SyncError::NotAuthorized);
       }
       ProvenanceResult::InvalidSignature => {
           return Err(SyncError::InvalidSignature);
       }
       ProvenanceResult::AuthorNotInShares => {
           return Err(SyncError::AuthorNotAuthorized);
       }
   }
   ```

3. **Validate entire manifest chain**

   When downloading a manifest chain, validate each manifest:

   ```rust
   async fn download_manifest_chain(...) -> Result<Vec<Manifest>> {
       let mut chain = Vec::new();

       for link in links {
           let manifest = download_manifest(link).await?;

           // Validate each manifest in the chain
           let result = verify_provenance(peer, &manifest)?;
           if !matches!(result, ProvenanceResult::Valid | ProvenanceResult::UnsignedLegacy) {
               return Err(SyncError::InvalidManifestInChain { link, reason: result });
           }

           chain.push(manifest);
       }

       Ok(chain)
   }
   ```

   **Important:** Checking the author is in the *current* manifest's shares is not sufficient.
   An attacker could craft a manifest that adds themselves to shares. The author's permissions
   must be validated against the *previous* manifest in the chain (see Ticket 2 for role
   enforcement). For each manifest N, verify the author was an Owner in manifest N-1.

4. **Add new error types** (`crates/common/src/peer/sync/error.rs` or equivalent)

   ```rust
   pub enum SyncError {
       // ... existing variants ...
       InvalidSignature,
       AuthorNotAuthorized,
       InvalidManifestInChain { link: Link, reason: String },
   }
   ```

## Files to Modify

| File | Changes |
|------|---------|
| `crates/common/src/peer/sync/sync_bucket.rs` | Update `verify_provenance()`, add chain validation |
| `crates/common/src/peer/sync/mod.rs` | Add new error types |

## Acceptance Criteria

- [ ] `verify_provenance()` checks signature validity
- [ ] `verify_provenance()` checks author is in shares
- [ ] Invalid signatures cause sync rejection
- [ ] Authors not in shares cause sync rejection
- [ ] Unsigned legacy manifests are accepted with warning (migration mode)
- [ ] Entire manifest chain is validated, not just final manifest
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings

## Verification

```rust
#[tokio::test]
async fn test_sync_rejects_invalid_signature() {
    // Setup two peers
    let owner = setup_peer().await;
    let receiver = setup_peer().await;

    // Create bucket shared with receiver
    let bucket = create_bucket(&owner, &receiver).await;

    // Tamper with manifest after signing
    let mut manifest = bucket.manifest().clone();
    manifest.set_name("tampered".to_string());

    // Sync should fail
    let result = sync_bucket(&receiver, &manifest).await;
    assert!(matches!(result, Err(SyncError::InvalidSignature)));
}

#[tokio::test]
async fn test_sync_rejects_unknown_author() {
    let owner = setup_peer().await;
    let receiver = setup_peer().await;
    let attacker = setup_peer().await;

    // Create bucket shared with receiver (not attacker)
    let bucket = create_bucket(&owner, &receiver).await;

    // Attacker creates manifest signed with their key
    let mut fake_manifest = bucket.manifest().clone();
    fake_manifest.sign(&attacker.secret()).unwrap();

    // Sync should fail - attacker not in shares
    let result = sync_bucket(&receiver, &fake_manifest).await;
    assert!(matches!(result, Err(SyncError::AuthorNotAuthorized)));
}
```

## Migration Strategy

1. Deploy code that signs new manifests but accepts unsigned (Phase 1)
2. Wait for all peers to upgrade
3. Deploy code that rejects unsigned manifests (Phase 2)

The `UnsignedLegacy` variant allows backwards compatibility during migration.
