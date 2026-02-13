# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v0.1.1 (2026-02-13)

### Bug Fixes

 - <csr-id-227fb3f9fc3d4c82381ddf85643ceaf20afd6000/> correct crate publish order and add missing README
   - Reorder publish steps: object-store → common → daemon
   - Add README.md for jax-object-store (required by crates.io)

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 1 commit contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Correct crate publish order and add missing README ([`227fb3f`](https://github.com/jax-protocol/jax-fs/commit/227fb3f9fc3d4c82381ddf85643ceaf20afd6000))
</details>

## v0.1.0 (2026-02-13)

### New Features

 - <csr-id-30f511b983bf98d49081ef6aa6ad6e99b5c82c8f/> complete SQLite + S3 blob store with iroh-blobs integration
   * feat: implement iroh-blobs Store backend for S3 blob store
- Add S3Actor to handle all ~20 proto::Request command variants
- Add S3Store wrapper implementing iroh-blobs Store API
- Add bucket existence check on S3 initialization (fail-fast)
- Add ensure_bucket to bin/minio for auto-creation in dev
- Update e2e skill with sync timing guidance (60s wait)

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 2 unique issues were worked on: [#58](https://github.com/jax-protocol/jax-fs/issues/58), [#65](https://github.com/jax-protocol/jax-fs/issues/65)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#58](https://github.com/jax-protocol/jax-fs/issues/58)**
    - Complete SQLite + S3 blob store with iroh-blobs integration ([`30f511b`](https://github.com/jax-protocol/jax-fs/commit/30f511b983bf98d49081ef6aa6ad6e99b5c82c8f))
 * **[#65](https://github.com/jax-protocol/jax-fs/issues/65)**
    - Bump jax-object-store v0.1.0, jax-common v0.1.6, jax-daemon v0.1.7 ([`f0219f2`](https://github.com/jax-protocol/jax-fs/commit/f0219f2d882d65272b5cbe81a39680a06006a0d3))
</details>

<csr-unknown>
The S3 blob store now fully integrates with iroh-blobs protocol,enabling P2P sync with blobs stored in S3/MinIO.<csr-unknown/>

