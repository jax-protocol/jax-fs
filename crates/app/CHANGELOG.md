# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- next-header -->
## [Unreleased] - ReleaseDate

## [0.1.0] - 2025-10-12

### Added
- Initial release
- CLI tool for JaxBucket
- Encrypted storage bucket management

## v0.1.7 (2025-11-18)

### Bug Fixes

 - <csr-id-76d456262a6fa4f16b4dfb6e7e120ac057bc47da/> use gateway URL for download button instead of localhost API
   The download button was using the localhost API URL which doesn't work
   for remote read-only nodes that don't expose the API over the internet.
   Now it uses the same gateway URL pattern as the share button, ensuring
   downloads work consistently across all node types.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 1 commit contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 1 unique issue was worked on: [#20](https://github.com/jax-protocol/jax-buckets/issues/20)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#20](https://github.com/jax-protocol/jax-buckets/issues/20)**
    - Use gateway URL for download button instead of localhost API ([`76d4562`](https://github.com/jax-protocol/jax-buckets/commit/76d456262a6fa4f16b4dfb6e7e120ac057bc47da))
</details>

## v0.1.6 (2025-11-18)

<csr-id-ef5cd61f032d20ff42ea68caf22a4ac46355c137/>
<csr-id-d0a31f491f14927e4b5453daceeaafc963dd4171/>
<csr-id-20eab70de45b734acd0e44f4340dcb6659b32e84/>
<csr-id-1b2d7c55806152c9e67d452c90543966f1e6b7d6/>

### Chore

 - <csr-id-ef5cd61f032d20ff42ea68caf22a4ac46355c137/> bump jax-service and jax-bucket to 0.1.2
 - <csr-id-d0a31f491f14927e4b5453daceeaafc963dd4171/> updated readme reference
 - <csr-id-20eab70de45b734acd0e44f4340dcb6659b32e84/> update internal manifest versions

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

 - 5 commits contributed to the release.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 4 unique issues were worked on: [#13](https://github.com/jax-protocol/jax-buckets/issues/13), [#15](https://github.com/jax-protocol/jax-buckets/issues/15), [#16](https://github.com/jax-protocol/jax-buckets/issues/16), [#18](https://github.com/jax-protocol/jax-buckets/issues/18)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#13](https://github.com/jax-protocol/jax-buckets/issues/13)**
    - Buil upload fix ([`7445f94`](https://github.com/jax-protocol/jax-buckets/commit/7445f9401d0f2be279c025815018c43554f28103))
 * **[#15](https://github.com/jax-protocol/jax-buckets/issues/15)**
    - Bump jax-common v0.1.5, jax-bucket v0.1.6 ([`c239f47`](https://github.com/jax-protocol/jax-buckets/commit/c239f477f3353c779bb731b2027edde31598dad7))
 * **[#16](https://github.com/jax-protocol/jax-buckets/issues/16)**
    - Bump jax-common v0.1.5, jax-bucket v0.1.6 ([`a5d2374`](https://github.com/jax-protocol/jax-buckets/commit/a5d2374b45790c295d43f7c66159d46ac2c15bf4))
 * **[#18](https://github.com/jax-protocol/jax-buckets/issues/18)**
    - Bump jax-common v0.1.5, jax-bucket v0.1.6 ([`414464a`](https://github.com/jax-protocol/jax-buckets/commit/414464a83b79b34590fed77df3dd500fe22a59c2))
 * **Uncategorized**
    - Bump jax-common v0.1.5, jax-bucket v0.1.6 ([`96d3bb8`](https://github.com/jax-protocol/jax-buckets/commit/96d3bb8821d510e36c3385ce943afc3ca53fa547))
</details>

## v0.1.5 (2025-11-17)

<csr-id-ef5cd61f032d20ff42ea68caf22a4ac46355c137/>
<csr-id-d0a31f491f14927e4b5453daceeaafc963dd4171/>
<csr-id-20eab70de45b734acd0e44f4340dcb6659b32e84/>
<csr-id-1b2d7c55806152c9e67d452c90543966f1e6b7d6/>

### Chore

 - <csr-id-ef5cd61f032d20ff42ea68caf22a4ac46355c137/> bump jax-service and jax-bucket to 0.1.2
 - <csr-id-d0a31f491f14927e4b5453daceeaafc963dd4171/> updated readme reference
 - <csr-id-20eab70de45b734acd0e44f4340dcb6659b32e84/> update internal manifest versions

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

## v0.1.4 (2025-11-15)

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 1 commit contributed to the release.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Adjusting changelogs prior to release of jax-common v0.1.3, jax-bucket v0.1.4 ([`96c3c3f`](https://github.com/jax-protocol/jax-buckets/commit/96c3c3fdd170dcfa12c4c08f23b09d077ea543c2))
</details>

## v0.1.3 (2025-11-15)

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

## v0.1.2 (2025-10-13)

<csr-id-ef5cd61f032d20ff42ea68caf22a4ac46355c137/>
<csr-id-d0a31f491f14927e4b5453daceeaafc963dd4171/>
<csr-id-20eab70de45b734acd0e44f4340dcb6659b32e84/>

### Chore

 - <csr-id-ef5cd61f032d20ff42ea68caf22a4ac46355c137/> bump jax-service and jax-bucket to 0.1.2
 - <csr-id-d0a31f491f14927e4b5453daceeaafc963dd4171/> updated readme reference

### Chore

 - <csr-id-20eab70de45b734acd0e44f4340dcb6659b32e84/> update internal manifest versions

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 6 commits contributed to the release.
 - 3 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Bump jax-service and jax-bucket to 0.1.2 ([`ef5cd61`](https://github.com/jax-protocol/jax-buckets/commit/ef5cd61f032d20ff42ea68caf22a4ac46355c137))
    - Bump jax-service v0.1.1, jax-bucket v0.1.1 ([`b2c4a8c`](https://github.com/jax-protocol/jax-buckets/commit/b2c4a8cf0f99fcb329fbb0993ebb9e4a26285659))
    - Updated readme reference ([`d0a31f4`](https://github.com/jax-protocol/jax-buckets/commit/d0a31f491f14927e4b5453daceeaafc963dd4171))
    - Adjusting changelogs prior to release of jax-common v0.1.1, jax-service v0.1.1, jax-bucket v0.1.1 ([`e053057`](https://github.com/jax-protocol/jax-buckets/commit/e0530577122769502f93af02296d02430f5e1f13))
    - Update internal manifest versions ([`20eab70`](https://github.com/jax-protocol/jax-buckets/commit/20eab70de45b734acd0e44f4340dcb6659b32e84))
    - Chore: restructure workspace and setup   independent versioning ([`325e79b`](https://github.com/jax-protocol/jax-buckets/commit/325e79b23b66d0a086a639130ade90ba11fd4a4d))
</details>

## v0.1.1 (2025-10-12)

<csr-id-20eab70de45b734acd0e44f4340dcb6659b32e84/>
<csr-id-d0a31f491f14927e4b5453daceeaafc963dd4171/>

### Chore

 - <csr-id-20eab70de45b734acd0e44f4340dcb6659b32e84/> update internal manifest versions
 - <csr-id-d0a31f491f14927e4b5453daceeaafc963dd4171/> updated readme reference

