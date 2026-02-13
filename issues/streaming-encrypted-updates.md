# Streaming Encrypted File Updates

**Status:** Research / Future
**Related:** partial-blob-support.md, fuse-missing-operations.md

## Objective

Enable efficient append, truncate, and partial write operations on encrypted files without requiring full file re-encryption.

## Background

### Current Implementation

Files are encrypted as atomic units using ChaCha20-Poly1305 AEAD:
```
nonce (12 bytes) || encrypted(blake3_hash(32) || plaintext) || tag (16 bytes)
```

**Problem:** The Poly1305 authentication tag covers the entire ciphertext. Any modification requires:
1. Decrypt entire file
2. Modify in memory
3. Re-encrypt entire file with new secret
4. Store new blob

For a 1-byte append to a 1GB file, this means 2GB of I/O + full re-encryption.

### Insight

ChaCha20 and BLAKE3/bao are both **streaming-compatible**:

- **ChaCha20** is a stream cipher with seekable keystream: `keystream[N] = ChaCha20(key, nonce, counter=N/64)`. You can encrypt byte 1,000,000 without touching bytes 0-999,999.

- **Bao/BLAKE3** uses a Merkle tree: updating one chunk only requires rehashing O(log n) nodes to the root.

The limitation is **Poly1305**, which requires the full ciphertext. By separating encryption (ChaCha20) from integrity (bao tree), we can achieve streaming updates.

## Proposed Architecture

```
┌─────────────────────────────────────────────────────────┐
│  File: encrypted with ChaCha20 (stream cipher only)    │
│  ┌──────┬──────┬──────┬──────┬──────┬──────┐           │
│  │chunk0│chunk1│chunk2│chunk3│chunk4│chunk5│  16KB ea  │
│  └──┬───┴──┬───┴──┬───┴──┬───┴──┬───┴──┬───┘           │
│     │      │      │      │      │      │                │
│     └──┬───┴──────┼──────┴───┬──┘      │                │
│        │          │          │         │                │
│     ┌──┴──┐    ┌──┴──┐    ┌──┴──┐      │   Bao tree    │
│     │hash │    │hash │    │hash │──────┘   (integrity)  │
│     └──┬──┘    └──┬──┘    └──┬──┘                       │
│        └────┬─────┘         │                           │
│           ┌─┴─┐          ┌──┘                           │
│           │   │──────────┘                              │
│           └─┬─┘                                         │
│          root hash (stored in NodeLink)                 │
└─────────────────────────────────────────────────────────┘
```

### Chunking Strategy: Fixed vs Content-Defined

**Fixed-size chunks** (16KB, matches iroh default):
- Simple, predictable
- Good for append-only workloads
- Problem: insert/delete in middle shifts ALL subsequent chunks

**Content-defined chunking (CDC)** using Rabin fingerprinting:
- Chunk boundaries determined by plaintext content (rolling hash)
- Insert/delete only affects 1-2 chunks around the edit point
- Much better for documents, logs, files edited in the middle

```
Fixed chunks - insert 1 byte at position 1000:
[chunk0][chunk1][chunk2][chunk3]
         ^ insert here
[chunk0'][chunk1'][chunk2'][chunk3']  <- ALL chunks shift, all hashes change

CDC/Rabin - insert 1 byte:
[chunk0][chunk1][chunk2][chunk3]
         ^ insert here
[chunk0][chunk1'][chunk2][chunk3]     <- only affected chunk changes
```

**CDC Flow**:
1. Rabin-chunk the **plaintext** (boundaries based on content)
2. Encrypt each chunk independently with derived key
3. Build bao tree over encrypted chunks
4. Store chunk boundaries in metadata (or re-derive on read)

**Crates**: `fastcdc`, `gearhash` (used by restic)

**Tradeoff**: Chunk boundaries leak information about plaintext structure. Two files with identical boundary patterns may have similar content. Acceptable for P2P sync where file sizes are already visible.

**Recommendation**: Support both modes:
- Fixed chunks for streaming/append workloads (logs, media)
- CDC for document-like files that get edited in place

### Operation Costs

| Operation | Current | Proposed |
|-----------|---------|----------|
| Append N bytes | O(file_size) | O(N + log(chunks)) |
| Truncate | O(file_size) | O(affected_chunk + log(chunks)) |
| Random write | O(file_size) | O(chunk_size + log(chunks)) |
| Read range | O(file_size) | O(range_size) |

## Implementation Steps

### Phase 1: Chunked Storage Format

**Files:** `crates/common/src/mount/node.rs`, `crates/common/src/crypto/`

1. Add new `NodeLink` variant for chunked files:
   ```rust
   enum NodeLink {
       Data(Link, Secret, MaybeMime),      // Current: single encrypted blob
       Dir(Link, Secret),                   // Directory node
       ChunkedData {                        // New: streaming format
           root_hash: Hash,                 // Bao root (integrity)
           secret: Secret,                  // ChaCha20 key
           nonce: [u8; 12],                 // Fixed nonce, counter = chunk offset
           size: u64,
           chunk_size: u32,                 // Default 16KB (matches iroh)
           mime: MaybeMime,
       },
   }
   ```

2. Add `StreamingSecret` that uses ChaCha20 without Poly1305:
   ```rust
   impl StreamingSecret {
       fn encrypt_chunk(&self, chunk_index: u64, data: &[u8]) -> Vec<u8>;
       fn decrypt_chunk(&self, chunk_index: u64, data: &[u8]) -> Vec<u8>;
   }
   ```

### Phase 2: Mount Operations

**File:** `crates/common/src/mount/mount_inner.rs`

1. Add `append` operation:
   ```rust
   impl Mount {
       pub async fn append(&mut self, path: &Path, data: R) -> Result<(), MountError>;
   }
   ```

2. Add `truncate` operation:
   ```rust
   impl Mount {
       pub async fn truncate(&mut self, path: &Path, size: u64) -> Result<(), MountError>;
   }
   ```

3. Add `write_at` for random writes:
   ```rust
   impl Mount {
       pub async fn write_at(&mut self, path: &Path, offset: u64, data: &[u8]) -> Result<(), MountError>;
   }
   ```

### Phase 3: Integrate with iroh-blobs

**File:** `crates/object-store/src/`

- Use existing bao-tree infrastructure (already at 16KB chunk size)
- Store encrypted chunks as HashSeq
- Leverage `BlobFormat::HashSeq` for chunk collections

### Phase 4: FUSE Integration

**File:** `crates/daemon/src/fuse/jax_fs.rs`

- Update `handle_truncate` to use native truncate
- Implement `fallocate` for preallocation
- Optimize write buffers to use append when possible

## Security Considerations

1. **Nonce reuse**: With fixed nonce + counter scheme, must never reuse (key, nonce, counter) triple. Options:
   - Generate new key on any modification (current approach, safe)
   - Use chunk index as counter (safe if chunks are immutable once written)
   - Derive per-chunk nonce from (file_nonce, chunk_index)

2. **Integrity**: Bao tree provides equivalent security to HMAC - any tampering is detected. Root hash serves as commitment.

3. **Chunk boundaries**: Fixed chunk size prevents chosen-boundary attacks.

## Dependencies

- `bao-tree` v0.15+ (already present)
- `chacha20` (need to add standalone, currently only via `chacha20poly1305`)
- `fastcdc` or `gearhash` (for content-defined chunking)
- Partial blob support (see partial-blob-support.md)

## Acceptance Criteria

- [ ] ChunkedData NodeLink variant implemented
- [ ] StreamingSecret with seek-and-encrypt
- [ ] Mount.append() works without full file read
- [ ] Mount.truncate() works without full file read
- [ ] Bao root hash verified on read
- [ ] Backwards compatible with existing Data NodeLink
- [ ] FUSE operations use native methods when available
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings

## Open Questions

1. **Migration**: How to handle existing files? Convert on first write, or keep mixed formats?
2. **Small files**: Below what size threshold should we use atomic encryption (simpler, single blob)?
3. **Chunk size**: 16KB matches iroh default, but is it optimal for encrypted P2P sync?
4. **Key derivation**: Per-file key, or derive chunk keys from master + chunk index?
5. **Fixed vs CDC**: Per-file choice, or global setting? Could use MIME type heuristics (text/* → CDC, video/* → fixed).
6. **CDC parameters**: Target chunk size, min/max bounds? Typical values: 8KB target, 2KB min, 64KB max.
