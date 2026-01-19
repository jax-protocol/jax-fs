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

## v0.1.6 (2026-01-19)

### New Features

 - <csr-id-7af5ca16a8e0748a922a39e3e8fecb1a7411e3db/> add mirror principal role and bucket publishing workflow
   * feat: add mirror principal role and bucket publishing workflow
   
   Implement polymorphic principal roles (Owner and Mirror) with publishing:
   - Mirror principals can sync buckets but cannot decrypt until published
   - Extended /share endpoint with role parameter (defaults to owner)
   - Added /publish endpoint to grant mirrors decryption access
   - Mirrors start with Option<SecretShare> None until bucket is published
   - MirrorCannotMount error when unpublished mirror tries to load bucket
 - <csr-id-75f36dfd89913f4296dc1e9e8f0dd4b24d903fe7/> add path operation CRDT for conflict-free sync
   * feat: add path operation CRDT for conflict-free sync
   
   Introduce a lightweight Conflict-free Replicated Data Type (CRDT) to track
   filesystem path operations (add, remove, mkdir, mv) across peers. The operation
   log is stored as a separate encrypted blob (not in the manifest) to avoid
   leaking directory structure information. Enables deterministic conflict
   resolution during peer sync using Lamport timestamps and peer IDs.
   
   🤖 Generated with [Claude Code](https://claude.com/claude-code)
 - <csr-id-b30cb13139cc12ec1d4f31e2e8d14cfcfbf00865/> add mv operation to Mount
   * feat: add mv operation to Mount for moving/renaming files and directories
   
   Adds a new `mv` method to the Mount struct that allows moving or renaming
   files and directories. The operation preserves the existing NodeLink (no
   re-encryption of content needed), creates intermediate directories if
   needed, and properly tracks all new node hashes in pins.
   
   🤖 Generated with [Claude Code](https://claude.com/claude-code)

### Bug Fixes

 - <csr-id-2edfaf0ccb6fd91c08e5676385a5e2ec732040b8/> sync from available peers instead of failing if one is offline
   * fix: sync from available peers instead of failing if one is offline
   
   Allow sync operations to work with multiple peers from bucket shares,
   falling back to other peers if the preferred one is unreachable. This
   fixes the bug where sync fails entirely if not all peers are online.
   
   🤖 Generated with [Claude Code](https://claude.com/claude-code)

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release over the course of 28 calendar days.
 - 61 days passed between releases.
 - 4 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 4 unique issues were worked on: [#24](https://github.com/jax-protocol/jax-buckets/issues/24), [#27](https://github.com/jax-protocol/jax-buckets/issues/27), [#32](https://github.com/jax-protocol/jax-buckets/issues/32), [#36](https://github.com/jax-protocol/jax-buckets/issues/36)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#24](https://github.com/jax-protocol/jax-buckets/issues/24)**
    - Sync from available peers instead of failing if one is offline ([`2edfaf0`](https://github.com/jax-protocol/jax-buckets/commit/2edfaf0ccb6fd91c08e5676385a5e2ec732040b8))
 * **[#27](https://github.com/jax-protocol/jax-buckets/issues/27)**
    - Add mv operation to Mount ([`b30cb13`](https://github.com/jax-protocol/jax-buckets/commit/b30cb13139cc12ec1d4f31e2e8d14cfcfbf00865))
 * **[#32](https://github.com/jax-protocol/jax-buckets/issues/32)**
    - Add path operation CRDT for conflict-free sync ([`75f36df`](https://github.com/jax-protocol/jax-buckets/commit/75f36dfd89913f4296dc1e9e8f0dd4b24d903fe7))
 * **[#36](https://github.com/jax-protocol/jax-buckets/issues/36)**
    - Add mirror principal role and bucket publishing workflow ([`7af5ca1`](https://github.com/jax-protocol/jax-buckets/commit/7af5ca16a8e0748a922a39e3e8fecb1a7411e3db))
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
 - 3 unique issues were worked on: [#15](https://github.com/jax-protocol/jax-buckets/issues/15), [#16](https://github.com/jax-protocol/jax-buckets/issues/16), [#18](https://github.com/jax-protocol/jax-buckets/issues/18)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#15](https://github.com/jax-protocol/jax-buckets/issues/15)**
    - Bump jax-common v0.1.5, jax-bucket v0.1.6 ([`c239f47`](https://github.com/jax-protocol/jax-buckets/commit/c239f477f3353c779bb731b2027edde31598dad7))
 * **[#16](https://github.com/jax-protocol/jax-buckets/issues/16)**
    - Bump jax-common v0.1.5, jax-bucket v0.1.6 ([`a5d2374`](https://github.com/jax-protocol/jax-buckets/commit/a5d2374b45790c295d43f7c66159d46ac2c15bf4))
 * **[#18](https://github.com/jax-protocol/jax-buckets/issues/18)**
    - Bump jax-common v0.1.5, jax-bucket v0.1.6 ([`414464a`](https://github.com/jax-protocol/jax-buckets/commit/414464a83b79b34590fed77df3dd500fe22a59c2))
 * **Uncategorized**
    - Bump jax-common v0.1.5, jax-bucket v0.1.6 ([`96d3bb8`](https://github.com/jax-protocol/jax-buckets/commit/96d3bb8821d510e36c3385ce943afc3ca53fa547))
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
 - 2 unique issues were worked on: [#11](https://github.com/jax-protocol/jax-buckets/issues/11), [#12](https://github.com/jax-protocol/jax-buckets/issues/12)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#11](https://github.com/jax-protocol/jax-buckets/issues/11)**
    - Alex/misc fixes ([`2fb5ea6`](https://github.com/jax-protocol/jax-buckets/commit/2fb5ea6e39a4f4d1cdfb9668511fabe731a22e92))
 * **[#12](https://github.com/jax-protocol/jax-buckets/issues/12)**
    - Bump jax-common v0.1.4, jax-bucket v0.1.5 ([`9517f35`](https://github.com/jax-protocol/jax-buckets/commit/9517f35911441ae4b7ce93c75774b1cdb47a7731))
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
    - Adjusting changelogs prior to release of jax-common v0.1.3, jax-bucket v0.1.4 ([`96c3c3f`](https://github.com/jax-protocol/jax-buckets/commit/96c3c3fdd170dcfa12c4c08f23b09d077ea543c2))
    - Bump jax-common v0.1.2 ([`e1d5272`](https://github.com/jax-protocol/jax-buckets/commit/e1d5272f93e6b1eeb60c0ccbf4976a5247fdc952))
    - Own ([`2f3e70f`](https://github.com/jax-protocol/jax-buckets/commit/2f3e70f535b5aff4a13ea4df9bbf59047d0dd8c9))
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
 - 1 unique issue was worked on: [#5](https://github.com/jax-protocol/jax-buckets/issues/5)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#5](https://github.com/jax-protocol/jax-buckets/issues/5)**
    - Consolidate peer state management into unified architecture ([`1b2d7c5`](https://github.com/jax-protocol/jax-buckets/commit/1b2d7c55806152c9e67d452c90543966f1e6b7d6))
 * **Uncategorized**
    - Bump jax-common v0.1.2, jax-bucket v0.1.3 ([`625a2eb`](https://github.com/jax-protocol/jax-buckets/commit/625a2eb01786f8367e0446da8420c233447c0793))
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
    - Adjusting changelogs prior to release of jax-common v0.1.1, jax-service v0.1.2, jax-bucket v0.1.2 ([`7cb3b73`](https://github.com/jax-protocol/jax-buckets/commit/7cb3b737b9febdcc7612cf9b827b7b63ee9fbb4f))
    - Adjusting changelogs prior to release of jax-common v0.1.1, jax-service v0.1.1, jax-bucket v0.1.1 ([`e053057`](https://github.com/jax-protocol/jax-buckets/commit/e0530577122769502f93af02296d02430f5e1f13))
    - Chore: restructure workspace and setup   independent versioning ([`325e79b`](https://github.com/jax-protocol/jax-buckets/commit/325e79b23b66d0a086a639130ade90ba11fd4a4d))
</details>

