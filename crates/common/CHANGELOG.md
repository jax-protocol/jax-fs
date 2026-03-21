# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- next-header -->
## [Unreleased] - ReleaseDate

## [0.1.0] - 2025-10-12

### Added
- Initial release
- Core data structures and cryptography
- End-to-end encrypted P2P storage primitives

## v0.1.11 (2026-03-21)

### New Features

 - <csr-id-9c8bcce6898c669f99db2de72afeda54f7d82555/> response cache + image transform
   * feat(gateway): add two-tier response cache and image transform
   
   Add a gateway response cache that eliminates repeated tree traversal,
   decryption, and image transformation on cache hit.
   
   Cache architecture:
   - Layer 1 (Path Index): SQLite-backed mapping of (bucket_id, height,
     path, transform_params) to content hash
   - Layer 2 (Content Store): BLAKE3-addressed blob store for
     decrypted/transformed content, naturally deduplicated
   - Background actor for periodic eviction (old heights, LRU, TTL)
   
   Image transform API via query params on existing gateway routes:
   - ?w=400 — resize to width (maintains aspect ratio)
   - ?w=400&h=300 — resize to exact dimensions
   - ?q=75 — output quality 1-100 (JPEG/WebP)
   - Supports JPEG, PNG, WebP, GIF
   - Caps dimensions at 4096px, validates params
   
   Cached/transformed responses include Cache-Control: public,
   max-age=31536000, immutable.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 1 commit contributed to the release.
 - 2 days passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 1 unique issue was worked on: [#156](https://github.com/jax-protocol/jax-fs/issues/156)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#156](https://github.com/jax-protocol/jax-fs/issues/156)**
    - Response cache + image transform ([`9c8bcce`](https://github.com/jax-protocol/jax-fs/commit/9c8bcce6898c669f99db2de72afeda54f7d82555))
</details>

## v0.1.10 (2026-03-19)

### New Features

 - <csr-id-bcfb790dca89b970bc8091f9a9093274932a57be/> bucket allowlist with approval, removal, and sync filtering
   * feat: add bucket allowlist with approval, removal, and sync filtering
- Add bucket_status table with migration
- Extend BucketLogProvider trait with should_sync_content,
     on_new_bucket_discovered, and list_syncable_buckets
- Gate pin/blob downloads on bucket status in sync_bucket
- Filter periodic pings to only active buckets
- New POST /api/v0/bucket/approve and /ignore endpoints
- Add status field and filter to bucket list API
- Auto-set active status on self-created buckets
- Unmount FUSE mounts when ignoring a bucket

### Bug Fixes

 - <csr-id-56fece5340a6a1dda8afad8c651baa9f41d6591d/> prevent ping job queue saturation from blocking syncs
   Stale peer pings with ~30s connect timeouts were processed serially,
   starving sync/download jobs and flooding the bounded queue. This change:
   
   - Adds 5s timeout to ping operations (peer unavailability returns Ok)
- Spawns ping jobs concurrently (semaphore-capped at 10) instead of
     blocking the worker loop
- Increases periodic ping interval from 60s to 5 minutes
- Skips periodic batch if previous is still in flight

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 9 days passed between releases.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 3 unique issues were worked on: [#141](https://github.com/jax-protocol/jax-fs/issues/141), [#142](https://github.com/jax-protocol/jax-fs/issues/142), [#152](https://github.com/jax-protocol/jax-fs/issues/152)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#141](https://github.com/jax-protocol/jax-fs/issues/141)**
    - Prevent ping job queue saturation from blocking syncs ([`56fece5`](https://github.com/jax-protocol/jax-fs/commit/56fece5340a6a1dda8afad8c651baa9f41d6591d))
 * **[#142](https://github.com/jax-protocol/jax-fs/issues/142)**
    - Bucket allowlist with approval, removal, and sync filtering ([`bcfb790`](https://github.com/jax-protocol/jax-fs/commit/bcfb790dca89b970bc8091f9a9093274932a57be))
 * **[#152](https://github.com/jax-protocol/jax-fs/issues/152)**
    - Bump jax-common v0.1.10, jax-daemon v0.1.14 ([`bfdbca1`](https://github.com/jax-protocol/jax-fs/commit/bfdbca1b53b99e3ad00833c2cf909afe368085a7))
</details>

<csr-unknown>
Introduces a bucket status model (pending/active/ignored) so peers canapprove incoming shares before syncing content, and ignore buckets theydon’t want. Bucket log entries are preserved as an audit trail.<csr-unknown/>

## v0.1.9 (2026-03-09)

### New Features

 - <csr-id-e4f511b70d2b419cae83b2acc7d53a42ba58c0b4/> persist publish status across saves, add unpublish
   * feat(publish): persist publish status across saves, add unpublish

### Bug Fixes

 - <csr-id-258a9ef69fe3ec755fd4ed376cf68bd5e09ca8f9/> apply all manifests in chain instead of only the first
   apply_manifest_chain used `if let Some(...) = .iter().enumerate().next()`
   which only processed the first manifest and returned early. This caused
   sync to advance by only one height per 60s ping cycle, making cross-node
   sync extremely slow (N minutes for N heights instead of instant).
   
   Changed to a `for` loop so the entire manifest chain is applied in a
   single sync pass.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 3 unique issues were worked on: [#118](https://github.com/jax-protocol/jax-fs/issues/118), [#121](https://github.com/jax-protocol/jax-fs/issues/121), [#122](https://github.com/jax-protocol/jax-fs/issues/122)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#118](https://github.com/jax-protocol/jax-fs/issues/118)**
    - Persist publish status across saves, add unpublish ([`e4f511b`](https://github.com/jax-protocol/jax-fs/commit/e4f511b70d2b419cae83b2acc7d53a42ba58c0b4))
 * **[#121](https://github.com/jax-protocol/jax-fs/issues/121)**
    - Apply all manifests in chain instead of only the first ([`258a9ef`](https://github.com/jax-protocol/jax-fs/commit/258a9ef69fe3ec755fd4ed376cf68bd5e09ca8f9))
 * **[#122](https://github.com/jax-protocol/jax-fs/issues/122)**
    - Bump jax-common v0.1.9, jax-daemon v0.1.11 ([`3b7f495`](https://github.com/jax-protocol/jax-fs/commit/3b7f4951a2e5c7ad7a232c80bc997b2d7aef3886))
</details>

## v0.1.8 (2026-02-20)

### New Features

 - <csr-id-c339f04cd771efb6195c1779d9bd29b7a55027c7/> make blobs store configurable (separate paths + max import size)
   * feat: make blobs store configurable with separate DB/object paths and max import size

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release.
 - 2 days passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 2 unique issues were worked on: [#110](https://github.com/jax-protocol/jax-fs/issues/110), [#115](https://github.com/jax-protocol/jax-fs/issues/115)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#110](https://github.com/jax-protocol/jax-fs/issues/110)**
    - Make blobs store configurable (separate paths + max import size) ([`c339f04`](https://github.com/jax-protocol/jax-fs/commit/c339f04cd771efb6195c1779d9bd29b7a55027c7))
 * **[#115](https://github.com/jax-protocol/jax-fs/issues/115)**
    - Bump jax-object-store v0.1.3, jax-common v0.1.8, jax-daemon v0.1.10 ([`9eb3ccd`](https://github.com/jax-protocol/jax-fs/commit/9eb3ccd4612ed3a88d82f01e7055e30a0bb69c54))
</details>

## v0.1.7 (2026-02-17)

### New Features

 - <csr-id-c3abb856836a0e904cd487170abea4a37cf15a54/> add bucket publish CLI command
   - Add `jax bucket publish --bucket-id <UUID>` subcommand

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 3 unique issues were worked on: [#105](https://github.com/jax-protocol/jax-fs/issues/105), [#85](https://github.com/jax-protocol/jax-fs/issues/85), [#86](https://github.com/jax-protocol/jax-fs/issues/86)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#105](https://github.com/jax-protocol/jax-fs/issues/105)**
    - Bump jax-object-store v0.1.2, jax-common v0.1.7, jax-daemon v0.1.9 ([`32b5b30`](https://github.com/jax-protocol/jax-fs/commit/32b5b3096ad278823f98ed59917a7a2401e78b15))
 * **[#85](https://github.com/jax-protocol/jax-fs/issues/85)**
    - Add bucket publish CLI command ([`c3abb85`](https://github.com/jax-protocol/jax-fs/commit/c3abb856836a0e904cd487170abea4a37cf15a54))
 * **[#86](https://github.com/jax-protocol/jax-fs/issues/86)**
    - Add share removal for bucket owners ([`4ec1d14`](https://github.com/jax-protocol/jax-fs/commit/4ec1d14a91b6b7dde5b6945aa9b62b93f8ae5dca))
</details>

## v0.1.6 (2026-02-13)

### New Features

<csr-id-0a6d4fe6379ad7b96bf2f2169fb70d4e7d05f5bc/>
<csr-id-b62a25cf7f6b86d18a262281127fa16d94d6ed58/>
<csr-id-cabccaca7a0cbd91b294d5d96a1cc9992c8ffef3/>
<csr-id-7f4dcb71a245455d6818b117bcea4ac76ac677c8/>
<csr-id-7af5ca16a8e0748a922a39e3e8fecb1a7411e3db/>
<csr-id-75f36dfd89913f4296dc1e9e8f0dd4b24d903fe7/>
<csr-id-b30cb13139cc12ec1d4f31e2e8d14cfcfbf00865/>

 - <csr-id-30f511b983bf98d49081ef6aa6ad6e99b5c82c8f/> complete SQLite + S3 blob store with iroh-blobs integration
   * feat: implement iroh-blobs Store backend for S3 blob store
* feat: add sync validation for signed manifests
- Check signature is valid
- Check author was in previous manifest's shares (prevents self-authorization)
- Validate entire manifest chain, not just the latest
- Accept unsigned manifests with warning (migration mode)
* feat: add pluggable conflict resolution for PathOpLog merges
- LastWriteWins: Higher timestamp wins (default CRDT behavior)
- BaseWins: Local operations always win
- ForkOnConflict: Keep both, return unresolved conflicts
* feat: add jax-blobs-store crate with SQLite + object storage backend
- SQLite for metadata (hash, size, state, timestamps)
- Pluggable object storage backends (S3/MinIO/local/memory)
- Content-addressed storage using BLAKE3 hashes (iroh-blobs compatible)
- Recovery support to rebuild metadata from object storage
- Add ManifestError type for signing/verification errors
- Add sign() and verify_signature() methods to Manifest
- Sign manifests automatically in Mount::init() and Mount::save()
- Store SecretKey in MountInner for signing
- Enable serde feature for ed25519-dalek
- Add comprehensive unit tests for signing and tamper detection
* feat: add mirror principal role and bucket publishing workflow
- Mirror principals can sync buckets but cannot decrypt until published
- Extended /share endpoint with role parameter (defaults to owner)
- Added /publish endpoint to grant mirrors decryption access
- Mirrors start with Option<SecretShare> None until bucket is published
- MirrorCannotMount error when unpublished mirror tries to load bucket
* feat: add path operation CRDT for conflict-free sync
* feat: add mv operation to Mount for moving/renaming files and directories

### Bug Fixes

 - <csr-id-2edfaf0ccb6fd91c08e5676385a5e2ec732040b8/> sync from available peers instead of failing if one is offline
   * fix: sync from available peers instead of failing if one is offline

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 10 commits contributed to the release.
 - 9 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 10 unique issues were worked on: [#24](https://github.com/jax-protocol/jax-fs/issues/24), [#27](https://github.com/jax-protocol/jax-fs/issues/27), [#32](https://github.com/jax-protocol/jax-fs/issues/32), [#36](https://github.com/jax-protocol/jax-fs/issues/36), [#49](https://github.com/jax-protocol/jax-fs/issues/49), [#50](https://github.com/jax-protocol/jax-fs/issues/50), [#52](https://github.com/jax-protocol/jax-fs/issues/52), [#57](https://github.com/jax-protocol/jax-fs/issues/57), [#58](https://github.com/jax-protocol/jax-fs/issues/58), [#65](https://github.com/jax-protocol/jax-fs/issues/65)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#24](https://github.com/jax-protocol/jax-fs/issues/24)**
    - Sync from available peers instead of failing if one is offline ([`2edfaf0`](https://github.com/jax-protocol/jax-fs/commit/2edfaf0ccb6fd91c08e5676385a5e2ec732040b8))
 * **[#27](https://github.com/jax-protocol/jax-fs/issues/27)**
    - Add mv operation to Mount ([`b30cb13`](https://github.com/jax-protocol/jax-fs/commit/b30cb13139cc12ec1d4f31e2e8d14cfcfbf00865))
 * **[#32](https://github.com/jax-protocol/jax-fs/issues/32)**
    - Add path operation CRDT for conflict-free sync ([`75f36df`](https://github.com/jax-protocol/jax-fs/commit/75f36dfd89913f4296dc1e9e8f0dd4b24d903fe7))
 * **[#36](https://github.com/jax-protocol/jax-fs/issues/36)**
    - Add mirror principal role and bucket publishing workflow ([`7af5ca1`](https://github.com/jax-protocol/jax-fs/commit/7af5ca16a8e0748a922a39e3e8fecb1a7411e3db))
 * **[#49](https://github.com/jax-protocol/jax-fs/issues/49)**
    - Add pluggable conflict resolution for PathOpLog merges ([`b62a25c`](https://github.com/jax-protocol/jax-fs/commit/b62a25cf7f6b86d18a262281127fa16d94d6ed58))
 * **[#50](https://github.com/jax-protocol/jax-fs/issues/50)**
    - Add author and signature fields to Manifest ([`7f4dcb7`](https://github.com/jax-protocol/jax-fs/commit/7f4dcb71a245455d6818b117bcea4ac76ac677c8))
 * **[#52](https://github.com/jax-protocol/jax-fs/issues/52)**
    - Add SQLite + object storage blob store backend ([`cabccac`](https://github.com/jax-protocol/jax-fs/commit/cabccaca7a0cbd91b294d5d96a1cc9992c8ffef3))
 * **[#57](https://github.com/jax-protocol/jax-fs/issues/57)**
    - Add sync validation for signed manifests ([`0a6d4fe`](https://github.com/jax-protocol/jax-fs/commit/0a6d4fe6379ad7b96bf2f2169fb70d4e7d05f5bc))
 * **[#58](https://github.com/jax-protocol/jax-fs/issues/58)**
    - Complete SQLite + S3 blob store with iroh-blobs integration ([`30f511b`](https://github.com/jax-protocol/jax-fs/commit/30f511b983bf98d49081ef6aa6ad6e99b5c82c8f))
 * **[#65](https://github.com/jax-protocol/jax-fs/issues/65)**
    - Bump jax-object-store v0.1.0, jax-common v0.1.6, jax-daemon v0.1.7 ([`f0219f2`](https://github.com/jax-protocol/jax-fs/commit/f0219f2d882d65272b5cbe81a39680a06006a0d3))
</details>

## v0.1.5 (2025-11-18)

<csr-id-1b2d7c55806152c9e67d452c90543966f1e6b7d6/>

### Bug Fixes

 - <csr-id-2f3e70f535b5aff4a13ea4df9bbf59047d0dd8c9/> own

### Other

 - <csr-id-1b2d7c55806152c9e67d452c90543966f1e6b7d6/> Consolidate peer state management into unified architecture
   * fix: refacoted state
   
   * fix: better api
   
   * progress
   
   * saving work
   
   * fix: bucket log trait
   
   * saving work
   
   * fix: more refavctor
   
   * feat: job model
   
   * feat: intergrate new protocl peer into example service
   
   * fix: node back to running
   
   * feat: working demo again
   
   * fix: rm test data
   
   * chore: move peer builder to its own file
   
   * fix: split out sync managet into its own thing
   
   * feat: bunch of ui updates
   
   * feat: actual fucking file viewer
   
   * fix: oops
   
   * ci: fix
   
   * ci: fix
   
   * fix: video playing

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 3 unique issues were worked on: [#15](https://github.com/jax-protocol/jax-fs/issues/15), [#16](https://github.com/jax-protocol/jax-fs/issues/16), [#18](https://github.com/jax-protocol/jax-fs/issues/18)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#15](https://github.com/jax-protocol/jax-fs/issues/15)**
    - Bump jax-common v0.1.5, jax-bucket v0.1.6 ([`c239f47`](https://github.com/jax-protocol/jax-fs/commit/c239f477f3353c779bb731b2027edde31598dad7))
 * **[#16](https://github.com/jax-protocol/jax-fs/issues/16)**
    - Bump jax-common v0.1.5, jax-bucket v0.1.6 ([`a5d2374`](https://github.com/jax-protocol/jax-fs/commit/a5d2374b45790c295d43f7c66159d46ac2c15bf4))
 * **[#18](https://github.com/jax-protocol/jax-fs/issues/18)**
    - Bump jax-common v0.1.5, jax-bucket v0.1.6 ([`414464a`](https://github.com/jax-protocol/jax-fs/commit/414464a83b79b34590fed77df3dd500fe22a59c2))
 * **Uncategorized**
    - Bump jax-common v0.1.5, jax-bucket v0.1.6 ([`96d3bb8`](https://github.com/jax-protocol/jax-fs/commit/96d3bb8821d510e36c3385ce943afc3ca53fa547))
</details>

## v0.1.4 (2025-11-17)

<csr-id-1b2d7c55806152c9e67d452c90543966f1e6b7d6/>

### Bug Fixes

 - <csr-id-2f3e70f535b5aff4a13ea4df9bbf59047d0dd8c9/> own

### Other

 - <csr-id-1b2d7c55806152c9e67d452c90543966f1e6b7d6/> Consolidate peer state management into unified architecture
   * fix: refacoted state
   
   * fix: better api
   
   * progress
   
   * saving work
   
   * fix: bucket log trait
   
   * saving work
   
   * fix: more refavctor
   
   * feat: job model
   
   * feat: intergrate new protocl peer into example service
   
   * fix: node back to running
   
   * feat: working demo again
   
   * fix: rm test data
   
   * chore: move peer builder to its own file
   
   * fix: split out sync managet into its own thing
   
   * feat: bunch of ui updates
   
   * feat: actual fucking file viewer
   
   * fix: oops
   
   * ci: fix
   
   * ci: fix
   
   * fix: video playing

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release.
 - 2 days passed between releases.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 2 unique issues were worked on: [#11](https://github.com/jax-protocol/jax-fs/issues/11), [#12](https://github.com/jax-protocol/jax-fs/issues/12)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#11](https://github.com/jax-protocol/jax-fs/issues/11)**
    - Alex/misc fixes ([`2fb5ea6`](https://github.com/jax-protocol/jax-fs/commit/2fb5ea6e39a4f4d1cdfb9668511fabe731a22e92))
 * **[#12](https://github.com/jax-protocol/jax-fs/issues/12)**
    - Bump jax-common v0.1.4, jax-bucket v0.1.5 ([`9517f35`](https://github.com/jax-protocol/jax-fs/commit/9517f35911441ae4b7ce93c75774b1cdb47a7731))
</details>

## v0.1.3 (2025-11-15)

### Bug Fixes

 - <csr-id-2f3e70f535b5aff4a13ea4df9bbf59047d0dd8c9/> own

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Adjusting changelogs prior to release of jax-common v0.1.3, jax-bucket v0.1.4 ([`96c3c3f`](https://github.com/jax-protocol/jax-fs/commit/96c3c3fdd170dcfa12c4c08f23b09d077ea543c2))
    - Bump jax-common v0.1.2 ([`e1d5272`](https://github.com/jax-protocol/jax-fs/commit/e1d5272f93e6b1eeb60c0ccbf4976a5247fdc952))
    - Own ([`2f3e70f`](https://github.com/jax-protocol/jax-fs/commit/2f3e70f535b5aff4a13ea4df9bbf59047d0dd8c9))
</details>

## v0.1.2 (2025-11-15)

<csr-id-1b2d7c55806152c9e67d452c90543966f1e6b7d6/>

### Other

 - <csr-id-1b2d7c55806152c9e67d452c90543966f1e6b7d6/> Consolidate peer state management into unified architecture
   * fix: refacoted state
   
   * fix: better api
   
   * progress
   
   * saving work
   
   * fix: bucket log trait
   
   * saving work
   
   * fix: more refavctor
   
   * feat: job model
   
   * feat: intergrate new protocl peer into example service
   
   * fix: node back to running
   
   * feat: working demo again
   
   * fix: rm test data
   
   * chore: move peer builder to its own file
   
   * fix: split out sync managet into its own thing
   
   * feat: bunch of ui updates
   
   * feat: actual fucking file viewer
   
   * fix: oops
   
   * ci: fix
   
   * ci: fix
   
   * fix: video playing

### Bug Fixes

 - <csr-id-2f3e70f535b5aff4a13ea4df9bbf59047d0dd8c9/> own

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 1 unique issue was worked on: [#5](https://github.com/jax-protocol/jax-fs/issues/5)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#5](https://github.com/jax-protocol/jax-fs/issues/5)**
    - Consolidate peer state management into unified architecture ([`1b2d7c5`](https://github.com/jax-protocol/jax-fs/commit/1b2d7c55806152c9e67d452c90543966f1e6b7d6))
 * **Uncategorized**
    - Bump jax-common v0.1.2, jax-bucket v0.1.3 ([`625a2eb`](https://github.com/jax-protocol/jax-fs/commit/625a2eb01786f8367e0446da8420c233447c0793))
</details>

## v0.1.1 (2025-10-13)

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Adjusting changelogs prior to release of jax-common v0.1.1, jax-service v0.1.2, jax-bucket v0.1.2 ([`7cb3b73`](https://github.com/jax-protocol/jax-fs/commit/7cb3b737b9febdcc7612cf9b827b7b63ee9fbb4f))
    - Adjusting changelogs prior to release of jax-common v0.1.1, jax-service v0.1.1, jax-bucket v0.1.1 ([`e053057`](https://github.com/jax-protocol/jax-fs/commit/e0530577122769502f93af02296d02430f5e1f13))
    - Chore: restructure workspace and setup   independent versioning ([`325e79b`](https://github.com/jax-protocol/jax-fs/commit/325e79b23b66d0a086a639130ade90ba11fd4a4d))
</details>

