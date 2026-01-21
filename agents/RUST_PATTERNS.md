# Rust Patterns

This document describes the architectural patterns and conventions for Rust code in jax-bucket. Follow these patterns when adding new functionality or modifying existing code.

## Overview

jax-bucket follows idiomatic Rust patterns with:

- **Error handling** via `thiserror` and the `?` operator
- **Async operations** via `tokio`
- **Serialization** via `serde` with IPLD DAG-CBOR
- **Content-addressed storage** via iroh-blobs

---

## Error Handling

### Define Errors with thiserror

Each module defines its own error type using `thiserror`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MountError {
    #[error("path not found: {0}")]
    PathNotFound(String),

    #[error("path already exists: {0}")]
    PathAlreadyExists(String),

    #[error("mirror cannot mount: bucket is not published")]
    MirrorCannotMount,

    #[error("crypto error: {0}")]
    Crypto(#[from] CryptoError),

    #[error("blob error: {0}")]
    Blobs(#[from] BlobsError),
}
```

### Error Propagation

Use `?` for propagation, `#[from]` for automatic conversion:

```rust
pub async fn load(link: &Link, secret_key: &SecretKey, blobs: &BlobsStore) -> Result<Mount, MountError> {
    let manifest = Self::_get_manifest_from_blobs(link, blobs).await?;
    let share = manifest.get_share(&secret_key.public())
        .ok_or(MountError::ShareNotFound)?;

    if share.is_mirror() && !share.can_decrypt() {
        return Err(MountError::MirrorCannotMount);
    }

    let secret = share.share()
        .ok_or(MountError::MirrorCannotMount)?
        .recover(secret_key)?;

    // ...
}
```

### Library vs Application Errors

- **Library code** (`jax-common`): Use `thiserror`, be specific
- **Application code** (`jax-bucket`): Can use `anyhow` for top-level errors

---

## Async Patterns

### Use tokio for Async

All async code uses `tokio`:

```rust
use tokio::sync::Mutex;

pub struct Mount(Arc<Mutex<MountInner>>);

impl Mount {
    pub async fn inner(&self) -> tokio::sync::MutexGuard<'_, MountInner> {
        self.0.lock().await
    }

    pub async fn add<R>(&mut self, path: &Path, data: R) -> Result<(), MountError>
    where
        R: Read + Send + Sync + 'static + Unpin,
    {
        let mut inner = self.0.lock().await;
        // ...
    }
}
```

### Async Test Attribute

Use `#[tokio::test]` for async tests:

```rust
#[tokio::test]
async fn test_mirror_can_mount_published_bucket() {
    let blobs = BlobsStore::fs(&temp_path).await.unwrap();
    let mount = Mount::init(id, name, &key, &blobs).await.unwrap();
    // ...
}
```

---

## Serialization

### Use serde with DAG-CBOR

Data structures use `serde` with IPLD DAG-CBOR for content-addressed storage:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    shares: BTreeMap<String, Share>,
    pins: Link,
    entry: Link,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous: Option<Link>,
}
```

### Encoding/Decoding

```rust
use serde_ipld_dagcbor::{from_slice, to_vec};

// Encode
let bytes = to_vec(&manifest)?;

// Decode
let manifest: Manifest = from_slice(&bytes)?;
```

---

## Module Organization

### Standard Module Structure

```
module_name/
├── mod.rs           # Public exports and module declaration
├── types.rs         # Type definitions (if many)
├── error.rs         # Error types (if complex)
└── impl.rs          # Implementation (if large)
```

Or for simpler modules, everything in one file:

```rust
// mount_inner.rs

// Types
pub struct Mount(...);
pub struct MountInner { ... }

// Error
#[derive(Debug, Error)]
pub enum MountError { ... }

// Implementation
impl Mount {
    pub async fn init(...) -> Result<Self, MountError> { ... }
    pub async fn load(...) -> Result<Self, MountError> { ... }
    pub async fn save(...) -> Result<(...), MountError> { ... }
}

// Tests
#[cfg(test)]
mod test {
    use super::*;
    // ...
}
```

### Public API via mod.rs

```rust
// mount/mod.rs
mod manifest;
mod mount_inner;
mod node;
mod principal;

pub use manifest::{Manifest, Share};
pub use mount_inner::{Mount, MountError};
pub use node::{Node, NodeLink};
pub use principal::{Principal, PrincipalRole};
```

---

## Type Patterns

### Method Ordering

Organize `impl` blocks with methods in this order:

1. **Constructors** (`new`, `new_*`, `with_*`, `from_*`)
2. **Getters** (prefixed with `/* Getters */` comment)
3. **Setters/Mutators** (prefixed with `/* Setters */` comment)

```rust
impl Share {
    /// Create an owner share with an encrypted secret.
    pub fn new_owner(share: SecretShare, public_key: PublicKey) -> Self {
        Self {
            principal: Principal {
                role: PrincipalRole::Owner,
                identity: public_key,
            },
            share: Some(share),
        }
    }

    /// Create a mirror share (no secret until published).
    pub fn new_mirror(public_key: PublicKey) -> Self {
        Self {
            principal: Principal {
                role: PrincipalRole::Mirror,
                identity: public_key,
            },
            share: None,
        }
    }

    /* Getters */

    pub fn principal(&self) -> &Principal {
        &self.principal
    }

    pub fn share(&self) -> Option<&SecretShare> {
        self.share.as_ref()
    }

    pub fn role(&self) -> &PrincipalRole {
        &self.principal.role
    }

    pub fn is_mirror(&self) -> bool {
        self.principal.role == PrincipalRole::Mirror
    }

    pub fn is_owner(&self) -> bool {
        self.principal.role == PrincipalRole::Owner
    }

    pub fn can_decrypt(&self) -> bool {
        self.share.is_some()
    }

    /* Setters */

    pub fn set_share(&mut self, share: SecretShare) {
        self.share = Some(share);
    }
}
```

### Predicate Methods

Use `is_*` and `can_*` naming for boolean queries (these go in the getters section):

- `is_*` - checks state or type (`is_mirror`, `is_published`, `is_empty`)
- `can_*` - checks capability (`can_decrypt`, `can_write`)
- `has_*` - checks presence (`has_share`, `has_previous`)

---

## Testing Patterns

### Unit Tests in Same File

```rust
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_share_new_mirror() {
        let key = SecretKey::generate();
        let share = Share::new_mirror(key.public());
        assert!(share.is_mirror());
        assert!(!share.can_decrypt());
    }
}
```

### Integration Tests in tests/

For tests that need multiple modules or external resources:

```rust
// crates/common/tests/mount_tests.rs
use common::crypto::{Secret, SecretKey};
use common::mount::{Mount, MountError, PrincipalRole};

#[tokio::test]
async fn test_mirror_cannot_mount_unpublished_bucket() {
    // Setup, test, assertions...
}
```

### Test Helpers

Keep test setup DRY with helper functions:

```rust
async fn setup_test_env() -> (Mount, BlobsStore, SecretKey, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let blobs = BlobsStore::fs(&temp_dir.path().join("blobs")).await.unwrap();
    let key = SecretKey::generate();
    let mount = Mount::init(Uuid::new_v4(), "test".to_string(), &key, &blobs)
        .await.unwrap();
    (mount, blobs, key, temp_dir)
}
```

---

## API Design

### CLI Flag Hygiene

Prefer fewer, well-designed CLI flags over many granular ones:

**Bad** - Too many flags, env vars, runtime config for infrastructure:
```rust
#[arg(long)] blob_store: BlobStoreType,
#[arg(long)] s3_endpoint: Option<String>,
#[arg(long)] s3_bucket: Option<String>,
#[arg(long, env = "JAX_S3_ACCESS_KEY")] s3_access_key: Option<String>,
#[arg(long, env = "JAX_S3_SECRET_KEY")] s3_secret_key: Option<String>,
```

**Good** - Single URL, set at init time (infrastructure config):
```rust
#[arg(long)] s3_url: Option<String>,  // s3://key:secret@host:port/bucket
```

**Principles:**
- Infrastructure config (storage backend) → set at `init`, not runtime
- Runtime config (public hostname, external URLs) → can be CLI flags on daemon
- Credentials → prefer URL encoding or config file over env vars
- Combine related params into URLs or structured config

### Config Locality

Configuration should be set where it makes sense:
- `init` time: Storage backends, directories, ports, infrastructure
- `daemon` time: Public hostname, external URLs, runtime behavior
- Config file: Persisted settings that rarely change

Don't put init-time config as daemon flags.

---

## Module Size

### Keep Files Focused

Each file should have one clear responsibility. Signs a file needs splitting:

- **Multiple "setup" functions** for different subsystems
- **Mix of concerns** (config parsing + state management + business logic)
- **> 200 lines** with distinct logical sections

**Bad** - `state.rs` does setup for database AND blobs:
```rust
// state.rs - 200+ lines
async fn setup_blobs_store(...) { /* 100 lines */ }
impl State {
    pub async fn from_config(...) { /* uses both */ }
}
```

**Good** - Separate modules for each subsystem:
```
daemon/
├── database/      # Database setup, one responsibility
│   ├── mod.rs
│   └── sqlite.rs
├── blobs/         # Blobs setup, one responsibility
│   ├── mod.rs
│   └── setup.rs
└── state.rs       # Orchestrates both, delegates setup
```

### Follow Existing Patterns

When adding new subsystems, check how existing ones are structured:
- Does `database/` have a pattern? Follow it for `blobs/`
- Does the crate have a module per subsystem? Don't inline.

---

## Avoiding Dead Code

### Only Write What's Needed

Don't write speculative code. Every public method should have a caller.

**Bad** - Methods "for future use":
```rust
impl Blobs {
    pub fn store(&self) -> &BlobsStore { &self.0 }      // Never called
    pub fn into_inner(self) -> BlobsStore { self.0 }    // Actually used
}

impl Deref for Blobs {  // Never used via deref
    type Target = BlobsStore;
    fn deref(&self) -> &Self::Target { &self.0 }
}
```

**Good** - Only what's needed:
```rust
impl Blobs {
    pub fn into_inner(self) -> BlobsStore { self.0 }
}
```

**Before adding code, ask:**
1. Is there a caller for this right now?
2. Am I adding this "just in case"?
3. Will this show up as dead_code warning?

If `#[allow(dead_code)]` is needed, the code probably shouldn't exist yet.

---

## Quick Reference

| Pattern | Example |
|---------|---------|
| Error type | `#[derive(Debug, Error)] pub enum FooError` |
| Error variant | `#[error("message: {0}")] Variant(String)` |
| From conversion | `#[error("inner")] Inner(#[from] InnerError)` |
| Async function | `pub async fn foo() -> Result<T, E>` |
| Async test | `#[tokio::test] async fn test_foo()` |
| Serialize | `#[derive(Serialize, Deserialize)]` |
| Option field | `#[serde(skip_serializing_if = "Option::is_none")]` |
| Public export | `pub use module::Type;` |
| Method order | constructors → `/* Getters */` → `/* Setters */` |
| Predicate names | `is_*`, `can_*`, `has_*` |
