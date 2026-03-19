# Cryptography

This document describes the cryptographic primitives and protocols used in JaxBucket for identity, key sharing, and content encryption.

## Identity

Each peer has an **Ed25519 keypair** as their identity.

**Location**: `crates/common/src/crypto/keys.rs`

```rust
pub struct SecretKey(ed25519_dalek::SigningKey);  // 32 bytes
pub struct PublicKey(ed25519_dalek::VerifyingKey); // 32 bytes
```

**Properties:**
- **SecretKey**: Stored in `~/.config/jax/secret.pem` (PEM format)
- **PublicKey**: Derived from secret key, used as Node ID
- **Dual Purpose**:
  1. Network identity (Iroh uses PublicKey as NodeId)
  2. Encryption key sharing (converted to X25519 for ECDH)

**Key Generation:**
```rust
let secret_key = SecretKey::generate();
let public_key = secret_key.public_key();
```

## Key Sharing

Buckets are shared between peers using **ECDH + AES Key Wrap**.

**Location**: `crates/common/src/crypto/share.rs`

### Protocol

To share a bucket secret with another peer:

1. **Generate Ephemeral Key**: Create temporary Ed25519 keypair
2. **ECDH**: Convert both keys to X25519 and compute shared secret
   ```rust
   let shared_secret = ecdh(ephemeral_secret, recipient_public);
   ```
3. **AES Key Wrap**: Wrap the bucket secret using shared secret (RFC 3394)
   ```rust
   let wrapped = aes_kw::wrap(kek: shared_secret, secret: bucket_secret);
   ```
4. **Package Share**: Combine ephemeral public key + wrapped secret
   ```rust
   Share = [ephemeral_pubkey(32 bytes) || wrapped_secret(40 bytes)]
   // Total: 72 bytes
   ```

### Unwrapping

The recipient recovers the secret:

1. Extract ephemeral public key from Share (first 32 bytes)
2. Compute ECDH with their private key
   ```rust
   let shared_secret = ecdh(my_secret, ephemeral_public);
   ```
3. Unwrap the secret using AES-KW
   ```rust
   let bucket_secret = aes_kw::unwrap(kek: shared_secret, wrapped);
   ```

### BucketShare Structure

```rust
pub struct BucketShare {
    pub principal: Principal,
    pub share: Share,
}

pub struct Principal {
    pub role: PrincipalRole,  // Owner, Editor, Viewer
    pub identity: PublicKey,  // Peer's public key
}

pub enum PrincipalRole {
    Owner,   // Full control
    Editor,  // Read + Write
    Viewer,  // Read only
}
```

## Content Encryption

Files and nodes are encrypted with **ChaCha20-Poly1305 AEAD**.

**Location**: `crates/common/src/crypto/secret.rs`

```rust
pub struct Secret([u8; 32]);  // 256-bit key
```

### Encryption Process

1. **Generate Nonce**: Random 96-bit nonce (12 bytes)
2. **Encrypt**: Use ChaCha20-Poly1305
   ```rust
   let cipher = ChaCha20Poly1305::new(&secret);
   let ciphertext = cipher.encrypt(&nonce, plaintext)?;
   ```
3. **Format**: `nonce(12) || ciphertext || tag(16)`
4. **Hash**: Compute BLAKE3 hash of the encrypted blob
5. **Store**: Save blob with hash as address

### Properties

- **Per-Item Keys**: Each file and node has its own Secret
- **Content Addressing**: Hashes are stable (computed after encryption)
- **Fine-Grained Access**: Can share individual file keys without exposing entire bucket
- **Authentication**: AEAD provides tamper detection

### Decryption

1. Extract nonce (first 12 bytes)
2. Decrypt remaining bytes
   ```rust
   let plaintext = cipher.decrypt(&nonce, ciphertext)?;
   ```
3. Verify AEAD tag (automatic, failure = tampered data)

## Summary

| Component | Algorithm | Key Size | Purpose |
|-----------|-----------|----------|---------|
| Identity | Ed25519 | 256-bit | Peer identity, signatures |
| Key Exchange | X25519 ECDH | 256-bit | Derive shared secrets |
| Key Wrap | AES-KW (RFC 3394) | 256-bit | Wrap bucket secrets for sharing |
| Content Encryption | ChaCha20-Poly1305 | 256-bit | Encrypt files and nodes |
| Hashing | BLAKE3 | 256-bit | Content addressing |
