# Key Concepts

High-level concepts for understanding jax-bucket's architecture.

## Principals and Roles

### PrincipalRole

Every principal (user) in a bucket has a role:

- **Owner**: Full read/write access. Receives an encrypted `SecretShare` of the bucket's encryption key.
- **Mirror**: Read-only access after the bucket is published. Uses the manifest's `published_secret` for decryption.

### Share

A `Share` represents a principal's access to a bucket:

```rust
struct Share {
    principal: Principal,      // Identity and role
    share: Option<SecretShare>, // Encrypted key share (owners only)
}
```

- Owners always have a `SecretShare` encrypted to their public key
- Mirrors never have individual shares - they use `published_secret` from the manifest

## Manifest

The `Manifest` is the root metadata for a bucket:

```rust
struct Manifest {
    id: Uuid,                          // Bucket identifier
    name: String,                      // Human-readable name
    shares: BTreeMap<String, Share>,   // Access control list
    entry: Link,                       // Root directory node
    pins: Link,                        // Pinned content
    published_secret: Option<Secret>,  // Plaintext secret when published
    // ...
}
```

### Publishing

When a bucket is **published**, the encryption secret is stored in plaintext in the manifest:

- Before publish: Only owners can decrypt (via their individual `SecretShare`)
- After publish: Anyone with the manifest can decrypt (via `published_secret`)
- Publishing is **permanent** - once the secret is out, it cannot be revoked

## Storage

### BlobsStore

Content-addressed storage via iroh-blobs. All data is identified by its BLAKE3 hash.

```rust
let hash = blobs.put(data).await?;  // Store
let data = blobs.get(&hash).await?; // Retrieve
```

### Encryption

All bucket content is encrypted with ChaCha20-Poly1305:

- Each node/blob has its own `Secret` key
- Format: `nonce (12 bytes) || encrypted(hash || plaintext) || tag (16 bytes)`
- The BLAKE3 hash is prepended before encryption for content verification

### Links

A `Link` is a content-addressed pointer (hash) to encrypted data in the blob store.
