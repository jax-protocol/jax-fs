# Security

This document describes JaxBucket's security model, threat model, best practices, and implementation details.

## Threat Model

### JaxBucket Protects Against

**Untrusted Storage Providers**
- All blobs are encrypted
- Storage provider sees only hashes
- Cannot decrypt content without keys

**Passive Network Observers**
- QUIC provides TLS 1.3 encryption
- Peer connections are authenticated
- Traffic is encrypted end-to-end

**Unauthorized Peers**
- Only peers with valid BucketShare can decrypt
- ECDH ensures only recipient can unwrap secrets
- Access control enforced via shares list

**Tampered Data**
- AEAD detects modifications
- Content addressing ensures integrity
- Hash verification on all blobs

### JaxBucket Does NOT Protect Against

**Compromised Peer with Valid Access**
- If an authorized peer is compromised, attacker gains access
- No forward secrecy or key rotation (yet)
- Recommendation: Regularly audit shares list

**Malicious Authorized Peer**
- Authorized peers can leak data
- Trust model assumes peers with access are trustworthy
- Recommendation: Only share with trusted devices/users

**Metadata Leakage**
- Bucket structure visible (file count, sizes, hierarchy)
- Storage provider can see blob access patterns
- Recommendation: Use padding or cover traffic (future work)

**Traffic Analysis**
- Connection patterns may reveal peer relationships
- Sync frequency might leak activity patterns
- Recommendation: Use Tor or mixnets (future work)

**Side-Channel Attacks**
- Timing attacks on crypto operations
- Power analysis (if physical access)
- Recommendation: Use constant-time crypto (mostly implemented)

## Best Practices

1. **Protect Secret Keys**
   - Store `secret.pem` with `chmod 600`
   - Back up securely (encrypted, offline)
   - Never share or commit to version control

2. **Verify Peer Identity**
   - Check public key fingerprints out-of-band
   - Use QR codes or secure channels for initial sharing

3. **Regular Key Rotation**
   - Periodically rotate bucket secrets (manual process currently)
   - Remove old shares when no longer needed

4. **Audit Access**
   - Regularly review bucket shares
   - Remove peers that no longer need access

5. **Monitor Sync Activity**
   - Watch for unexpected updates
   - Investigate unknown peers or sync patterns

## Future Security Enhancements

- [ ] Forward secrecy via key rotation
- [ ] Access revocation with re-encryption
- [ ] Metadata padding to hide structure
- [ ] Traffic obfuscation
- [ ] Formal security audit

## Implementation Details

### Key Files

**Data Model:**
- **Manifest**: `crates/common/src/bucket/manifest.rs`
- **Node**: `crates/common/src/bucket/node.rs`
- **Pins**: `crates/common/src/bucket/pins.rs`
- **Bucket Log**: `crates/common/src/bucket_log/`
  - `provider.rs` - BucketLogProvider trait definition
  - `memory.rs` - In-memory implementation (testing/minimal peers)
  - Database implementations in app-specific crates

**Cryptography:**
- **Keys**: `crates/common/src/crypto/keys.rs`
- **Secret**: `crates/common/src/crypto/secret.rs`
- **Share**: `crates/common/src/crypto/share.rs`

**Peer & Sync:**
- **Link**: `crates/common/src/linked_data/link.rs`
- **Peer**: `crates/common/src/peer/mod.rs`
- **Protocol Messages**: `crates/common/src/peer/protocol/messages/`
  - `ping.rs` - Ping/Pong message definitions
  - `mod.rs` - Message router and handler registration
- **Sync Jobs**: `crates/common/src/peer/sync/jobs/`
  - `sync_bucket.rs` - Manifest chain download and log application
  - `download_pins.rs` - Pin content download
  - `mod.rs` - Job definitions and common utilities
- **Sync Provider**: `crates/daemon/src/daemon/sync_provider.rs`
  - QueuedSyncProvider implementation
  - Background worker with periodic ping scheduler

### Dependencies

- **Iroh**: P2P networking and blob storage
- **ed25519-dalek**: Identity keypairs
- **chacha20poly1305**: Content encryption
- **aes-kw**: Key wrapping (RFC 3394)
- **blake3**: Content addressing (via Iroh)
- **serde_ipld_dagcbor**: DAG-CBOR serialization

## References

- **Iroh**: https://iroh.computer/
- **IPLD**: https://ipld.io/
- **RFC 3394** (AES Key Wrap): https://tools.ietf.org/html/rfc3394
- **ChaCha20-Poly1305**: https://tools.ietf.org/html/rfc8439
- **Ed25519**: https://ed25519.cr.yp.to/
- **BLAKE3**: https://github.com/BLAKE3-team/BLAKE3
