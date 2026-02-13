# jax-object-store

SQLite + Object Storage backend for blob data.

## Overview

This crate provides an iroh-blobs compatible store implementation that uses SQLite for metadata and pluggable object storage (S3/MinIO/local filesystem/memory) for blob data.

## Features

- Content-addressed storage using BLAKE3 hashes (compatible with iroh-blobs)
- SQLite for fast metadata queries
- Multiple storage backends: S3, MinIO, local filesystem, in-memory
- Recovery support: rebuild metadata from object storage

## Usage

```rust
use jax_object_store::ObjectStore;
use std::path::Path;

// Create a local file-based store
let store = ObjectStore::new_local(Path::new("/tmp/blobs")).await?;

// Convert to iroh_blobs::api::Store for use with BlobsProtocol
let iroh_store: iroh_blobs::api::Store = store.into();
```

## License

MIT OR Apache-2.0
