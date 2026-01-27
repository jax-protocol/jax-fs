# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v0.1.0 (2026-01-27)

### New Features

 - <csr-id-cabccaca7a0cbd91b294d5d96a1cc9992c8ffef3/> add SQLite + object storage blob store backend
   * feat: add jax-blobs-store crate with SQLite + object storage backend
   
   New crate providing blob storage with:
   - SQLite for metadata (hash, size, state, timestamps)
   - Pluggable object storage backends (S3/MinIO/local/memory)
   - Content-addressed storage using BLAKE3 hashes (iroh-blobs compatible)
   - Recovery support to rebuild metadata from object storage

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 1 commit contributed to the release over the course of 5 calendar days.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 1 unique issue was worked on: [#52](https://github.com/jax-protocol/jax-buckets/issues/52)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#52](https://github.com/jax-protocol/jax-buckets/issues/52)**
    - Add SQLite + object storage blob store backend ([`cabccac`](https://github.com/jax-protocol/jax-buckets/commit/cabccaca7a0cbd91b294d5d96a1cc9992c8ffef3))
</details>

