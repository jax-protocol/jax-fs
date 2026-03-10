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

## v0.1.13 (2026-03-10)

### New Features

 - <csr-id-e71b5ac6767a27a4f11528368e72af83a13aab32/> shared CLI UI module with consistent formatting and --plain flag
   * feat: add shared CLI UI module with consistent formatting and --plain flag
   
   Add a centralized ui.rs module with status symbols (✓/→/✗/!), color
   helpers, string truncation, styled table builder, and plain output mode.
   All CLI commands now use consistent formatting via shared helpers instead
   of ad-hoc owo_colors calls. Tables use UTF8_FULL_CONDENSED preset with
   bold headers, truncated IDs/hashes, colored statuses, and colored roles.
   The --plain global flag disables colors and table borders for scripting.

### Bug Fixes

 - <csr-id-219d4fa9c69fa0c1cd9a4c53bb2edd898577cab5/> persist FUSE mutations (unlink, mkdir, rename, create) via SaveRequest
   The FUSE filesystem was not sending SaveRequest after unlink, mkdir,
   rename, and create operations, causing those changes to be lost on
   remount. Only flush (writes) and setattr (truncate) were persisting.
   
   Extract a request_save() helper method and call it from all mutation
   handlers so every FUSE operation that modifies the mount tree is
   persisted consistently.

### Refactor

 - <csr-id-ceb5cc9e512472c20cd60143467cbccc6a08ecd6/> route FUSE mutations through daemon HTTP API
   * refactor: route FUSE mutations through daemon HTTP API for persistence
   
   Replace the SaveRequest/save_tx channel mechanism with direct HTTP API
   calls to the daemon's endpoints. All FUSE mutations now persist via the
   same API that CLI and desktop clients use, ensuring a single source of
   truth for persistence.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 3 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 3 unique issues were worked on: [#127](https://github.com/jax-protocol/jax-fs/issues/127), [#130](https://github.com/jax-protocol/jax-fs/issues/130), [#131](https://github.com/jax-protocol/jax-fs/issues/131)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#127](https://github.com/jax-protocol/jax-fs/issues/127)**
    - Persist FUSE mutations (unlink, mkdir, rename, create) via SaveRequest ([`219d4fa`](https://github.com/jax-protocol/jax-fs/commit/219d4fa9c69fa0c1cd9a4c53bb2edd898577cab5))
 * **[#130](https://github.com/jax-protocol/jax-fs/issues/130)**
    - Shared CLI UI module with consistent formatting and --plain flag ([`e71b5ac`](https://github.com/jax-protocol/jax-fs/commit/e71b5ac6767a27a4f11528368e72af83a13aab32))
 * **[#131](https://github.com/jax-protocol/jax-fs/issues/131)**
    - Route FUSE mutations through daemon HTTP API ([`ceb5cc9`](https://github.com/jax-protocol/jax-fs/commit/ceb5cc9e512472c20cd60143467cbccc6a08ecd6))
</details>

## v0.1.12 (2026-03-09)

### Other

 - <csr-id-34ad157487cd460460fa8e8435d6946de347e439/> fmt

### New Features

 - <csr-id-fb9c937ff7c5119f3a229fd815fb17d6aeedda55/> add jax bucket stat CLI command
   Shows bucket name, ID, version hash, height, published status, and
   peer shares with roles.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 1 unique issue was worked on: [#123](https://github.com/jax-protocol/jax-fs/issues/123)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#123](https://github.com/jax-protocol/jax-fs/issues/123)**
    - Bump jax-daemon v0.1.12 ([`375af73`](https://github.com/jax-protocol/jax-fs/commit/375af73d5fabe4410889e214113137614bb2c705))
 * **Uncategorized**
    - Fmt ([`34ad157`](https://github.com/jax-protocol/jax-fs/commit/34ad157487cd460460fa8e8435d6946de347e439))
    - Add jax bucket stat CLI command ([`fb9c937`](https://github.com/jax-protocol/jax-fs/commit/fb9c937ff7c5119f3a229fd815fb17d6aeedda55))
</details>

## v0.1.11 (2026-03-09)

### New Features

 - <csr-id-e4f511b70d2b419cae83b2acc7d53a42ba58c0b4/> persist publish status across saves, add unpublish
   * feat(publish): persist publish status across saves, add unpublish

### Bug Fixes

 - <csr-id-dceb2c8c9f8f5e2b6121cf9a118f7773d4da3fd7/> each crate captures its own CARGO_PKG_VERSION
   Previously, `jax version` reported 0.1.8 (common's version) instead of
   0.1.10 (daemon's version) because BuildInfo::new() in common captured
   CARGO_PKG_VERSION at common's compile time.
   
   Now daemon and desktop each have their own version module that captures
   the correct version from their own compile environment.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 2 unique issues were worked on: [#118](https://github.com/jax-protocol/jax-fs/issues/118), [#122](https://github.com/jax-protocol/jax-fs/issues/122)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#118](https://github.com/jax-protocol/jax-fs/issues/118)**
    - Persist publish status across saves, add unpublish ([`e4f511b`](https://github.com/jax-protocol/jax-fs/commit/e4f511b70d2b419cae83b2acc7d53a42ba58c0b4))
 * **[#122](https://github.com/jax-protocol/jax-fs/issues/122)**
    - Bump jax-common v0.1.9, jax-daemon v0.1.11 ([`3b7f495`](https://github.com/jax-protocol/jax-fs/commit/3b7f4951a2e5c7ad7a232c80bc997b2d7aef3886))
 * **Uncategorized**
    - Each crate captures its own CARGO_PKG_VERSION ([`dceb2c8`](https://github.com/jax-protocol/jax-fs/commit/dceb2c8c9f8f5e2b6121cf9a118f7773d4da3fd7))
</details>

## v0.1.10 (2026-02-20)

### New Features

<csr-id-f4f23215f7fd92f09ccb7744c86387c1b97828a9/>
<csr-id-c339f04cd771efb6195c1779d9bd29b7a55027c7/>
<csr-id-d1166b1dc9359bfabeef9d5c2b6c70b5a5958f37/>

 - <csr-id-78fc49f5b9e96d4dd7dfe54a1a99ed544f69d33c/> add sidecar daemon support
   * feat(daemon): add history, is-published endpoints and ls `at` param
* feat(cli): rich output, consistent bucket resolution, and op system docs
- Add owo-colors, comfy-table, indicatif dependencies
- Extract resolve_bucket() helper for name-or-UUID resolution
- Convert all bucket commands to single positional <BUCKET> arg
- Create CLI wrapper structs for publish, shares commands
- Add typed output structs with styled Display for all commands
- Replace hand-rolled mount list table with comfy-table
- Remove mount list --json flag (use HTTP API for machine data)
- Add colored error chain formatting at the boundary
- Wire MultiProgress into OpContext for future spinners
- Update CLI.md with bucket resolution and typed output docs
- Reference CLI.md from PROJECT_LAYOUT.md
* feat: make blobs store configurable with separate DB/object paths and max import size
- Update ObjectStore::new_local to accept separate db_path and objects_path
     instead of deriving both from a single data_dir
- Make MAX_IMPORT_SIZE configurable via ObjectStoreActor instead of hardcoded
     constant, exposed as DEFAULT_MAX_IMPORT_SIZE (1GB)
- Add optional db_path field to BlobStoreConfig::Filesystem variant for
     separate SQLite metadata DB location
- Add max_import_size to AppConfig with serde default for backward compat
- Thread max_import_size through setup_blobs_store, Blobs::setup, and
     ServiceConfig to the actor
- Add *_with_max_import_size constructors to BlobsStore and ObjectStore
* feat: add CLI binary releases, install script, and desktop auto-updater
- Add release-cli.yml workflow to build and publish CLI binaries for
     macOS (arm64, x64) and Linux (x64) on jax-daemon-v* tags
- Add install.sh for one-line CLI install/update via curl
- Integrate tauri-plugin-updater for in-app desktop update checks
- Update release-desktop.yml to generate latest.json update manifest
     with signing support
- Add update check UI to Settings page in desktop app
- Update INSTALL.md and README.md with install script documentation

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 5 commits contributed to the release.
 - 2 days passed between releases.
 - 4 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 5 unique issues were worked on: [#107](https://github.com/jax-protocol/jax-fs/issues/107), [#109](https://github.com/jax-protocol/jax-fs/issues/109), [#110](https://github.com/jax-protocol/jax-fs/issues/110), [#112](https://github.com/jax-protocol/jax-fs/issues/112), [#115](https://github.com/jax-protocol/jax-fs/issues/115)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#107](https://github.com/jax-protocol/jax-fs/issues/107)**
    - Add sidecar daemon support ([`78fc49f`](https://github.com/jax-protocol/jax-fs/commit/78fc49f5b9e96d4dd7dfe54a1a99ed544f69d33c))
 * **[#109](https://github.com/jax-protocol/jax-fs/issues/109)**
    - Add CLI binary releases, install script, and desktop auto-updater ([`d1166b1`](https://github.com/jax-protocol/jax-fs/commit/d1166b1dc9359bfabeef9d5c2b6c70b5a5958f37))
 * **[#110](https://github.com/jax-protocol/jax-fs/issues/110)**
    - Make blobs store configurable (separate paths + max import size) ([`c339f04`](https://github.com/jax-protocol/jax-fs/commit/c339f04cd771efb6195c1779d9bd29b7a55027c7))
 * **[#112](https://github.com/jax-protocol/jax-fs/issues/112)**
    - Rich output, consistent bucket resolution, and op system ([`f4f2321`](https://github.com/jax-protocol/jax-fs/commit/f4f23215f7fd92f09ccb7744c86387c1b97828a9))
 * **[#115](https://github.com/jax-protocol/jax-fs/issues/115)**
    - Bump jax-object-store v0.1.3, jax-common v0.1.8, jax-daemon v0.1.10 ([`9eb3ccd`](https://github.com/jax-protocol/jax-fs/commit/9eb3ccd4612ed3a88d82f01e7055e30a0bb69c54))
</details>

## v0.1.9 (2026-02-17)

<csr-id-fc09685fd84e952ffc29ef5fbd150caa29a9395b/>

### New Features

<csr-id-e7a06101d010e4065849d8feef0ea82edf7a61c0/>

 - <csr-id-c3abb856836a0e904cd487170abea4a37cf15a54/> add bucket publish CLI command
   - Add `jax bucket publish --bucket-id <UUID>` subcommand

### Refactor

 - <csr-id-fc09685fd84e952ffc29ef5fbd150caa29a9395b/> clean up server module structure
   * refactor(http): restructure gateway module
   
   - Rename html/ → gateway/ to reflect actual content
   - Split monolithic 881-line handler into separate files:
     - mod.rs: router, mount loading, shared URL rewriting helpers
     - index.rs: gateway homepage (moved from gateway_index.rs)
     - directory.rs: self-contained directory listing handler
     - file.rs: self-contained file serving handler
   - Replace Accept header JSON detection with ?json query parameter
   - Remove ?view query parameter (redundant with default behavior)
   - Each handler file is self-contained with its own types and helpers

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 6 commits contributed to the release over the course of 2 calendar days.
 - 3 days passed between releases.
 - 3 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 6 unique issues were worked on: [#103](https://github.com/jax-protocol/jax-fs/issues/103), [#105](https://github.com/jax-protocol/jax-fs/issues/105), [#80](https://github.com/jax-protocol/jax-fs/issues/80), [#84](https://github.com/jax-protocol/jax-fs/issues/84), [#85](https://github.com/jax-protocol/jax-fs/issues/85), [#86](https://github.com/jax-protocol/jax-fs/issues/86)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#103](https://github.com/jax-protocol/jax-fs/issues/103)**
    - Clean up server module structure ([`fc09685`](https://github.com/jax-protocol/jax-fs/commit/fc09685fd84e952ffc29ef5fbd150caa29a9395b))
 * **[#105](https://github.com/jax-protocol/jax-fs/issues/105)**
    - Bump jax-object-store v0.1.2, jax-common v0.1.7, jax-daemon v0.1.9 ([`32b5b30`](https://github.com/jax-protocol/jax-fs/commit/32b5b3096ad278823f98ed59917a7a2401e78b15))
 * **[#80](https://github.com/jax-protocol/jax-fs/issues/80)**
    - Add negative cache and separate TTLs for FUSE performance ([`e7a0610`](https://github.com/jax-protocol/jax-fs/commit/e7a06101d010e4065849d8feef0ea82edf7a61c0))
 * **[#84](https://github.com/jax-protocol/jax-fs/issues/84)**
    - Add FUSE/non-FUSE desktop build separation ([`95bdccd`](https://github.com/jax-protocol/jax-fs/commit/95bdccd33f73d585a65fd8da4d84c718761c7915))
 * **[#85](https://github.com/jax-protocol/jax-fs/issues/85)**
    - Add bucket publish CLI command ([`c3abb85`](https://github.com/jax-protocol/jax-fs/commit/c3abb856836a0e904cd487170abea4a37cf15a54))
 * **[#86](https://github.com/jax-protocol/jax-fs/issues/86)**
    - Add share removal for bucket owners ([`4ec1d14`](https://github.com/jax-protocol/jax-fs/commit/4ec1d14a91b6b7dde5b6945aa9b62b93f8ae5dca))
</details>

## v0.1.8 (2026-02-14)

### New Features

<csr-id-63712a7a66ce843e31c3a300ed3159b3a9042e2f/>

 - <csr-id-c63681313cfb66b28eec389c1e7147bdfafad39d/> fix port default, add health/shares commands, gate mount behind fuse
   * feat(cli): fix port default, add health/shares commands, gate mount behind fuse
* feat(fuse): implement setattr and xattr stubs for FUSE compatibility
- setattr: handles truncate (size) and mtime changes
- handle_truncate helper: resizes files via write buffers or Mount
- xattr stubs (setxattr, getxattr, listxattr, removexattr): return
     ENOTSUP for macOS compatibility

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 3 unique issues were worked on: [#73](https://github.com/jax-protocol/jax-fs/issues/73), [#77](https://github.com/jax-protocol/jax-fs/issues/77), [#78](https://github.com/jax-protocol/jax-fs/issues/78)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#73](https://github.com/jax-protocol/jax-fs/issues/73)**
    - Implement missing FUSE operations for Unix command compatibility ([`63712a7`](https://github.com/jax-protocol/jax-fs/commit/63712a7a66ce843e31c3a300ed3159b3a9042e2f))
 * **[#77](https://github.com/jax-protocol/jax-fs/issues/77)**
    - Fix port default, add health/shares commands, gate mount behind fuse ([`c636813`](https://github.com/jax-protocol/jax-fs/commit/c63681313cfb66b28eec389c1e7147bdfafad39d))
 * **[#78](https://github.com/jax-protocol/jax-fs/issues/78)**
    - Bump jax-object-store v0.1.1, jax-daemon v0.1.8 ([`4311b03`](https://github.com/jax-protocol/jax-fs/commit/4311b03c6cb012b0e35a018750bbf03e6b574282))
</details>

## v0.1.7 (2026-02-13)

### New Features (BREAKING)

<csr-id-a413ee6c2157ffec2f39a9b2df6ea389e3988df2/>

 - <csr-id-ec12a4b6731782a787a29c90a440417916c26157/> add FUSE filesystem for mounting buckets as local directories
   * feat!: add FUSE filesystem for mounting buckets as local directories
* feat!: restructure daemon, add Tauri desktop app with full UI
- Remove Askama HTML UI (replaced by Tauri desktop app)
- Split HTTP server into run_api (private) and run_gateway (public)
- Export start_service + ShutdownHandle for embedding
- Add bucket_log history queries with published field
- Replace --app-port/--gateway-port with --api-port/--gateway-port
- Tauri backend with direct ServiceState IPC (no HTTP proxying)
- SolidJS frontend: Explorer, Viewer, Editor, History, Settings pages
- File explorer with breadcrumbs, upload, mkdir, delete, rename, move
- File viewer for text, markdown, images, video, audio
- Version history with read-only browsing of past versions
- Settings: auto-launch toggle, theme switcher, local config display
- SharePanel for per-bucket peer sharing from Explorer
- System tray with Open, Status, Quit
- Tauri capabilities for dialog and autostart permissions
- Separate CI (ci-tauri.yml) and release (release-desktop.yml) workflows

### Bug Fixes

 - <csr-id-76d456262a6fa4f16b4dfb6e7e120ac057bc47da/> use gateway URL for download button instead of localhost API
   The download button was using the localhost API URL which doesn't work
   for remote read-only nodes that don't expose the API over the internet.
   Now it uses the same gateway URL pattern as the share button, ensuring
   downloads work consistently across all node types.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 3 unique issues were worked on: [#62](https://github.com/jax-protocol/jax-fs/issues/62), [#64](https://github.com/jax-protocol/jax-fs/issues/64), [#65](https://github.com/jax-protocol/jax-fs/issues/65)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#62](https://github.com/jax-protocol/jax-fs/issues/62)**
    - Restructure daemon, add Tauri desktop app with full UI ([`a413ee6`](https://github.com/jax-protocol/jax-fs/commit/a413ee6c2157ffec2f39a9b2df6ea389e3988df2))
 * **[#64](https://github.com/jax-protocol/jax-fs/issues/64)**
    - Add FUSE filesystem for mounting buckets as local directories ([`ec12a4b`](https://github.com/jax-protocol/jax-fs/commit/ec12a4b6731782a787a29c90a440417916c26157))
 * **[#65](https://github.com/jax-protocol/jax-fs/issues/65)**
    - Bump jax-object-store v0.1.0, jax-common v0.1.6, jax-daemon v0.1.7 ([`f0219f2`](https://github.com/jax-protocol/jax-fs/commit/f0219f2d882d65272b5cbe81a39680a06006a0d3))
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

## v0.1.4 (2025-11-15)

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

## v0.1.2 (2025-10-13)

<csr-id-ef5cd61f032d20ff42ea68caf22a4ac46355c137/>
<csr-id-d0a31f491f14927e4b5453daceeaafc963dd4171/>
<csr-id-20eab70de45b734acd0e44f4340dcb6659b32e84/>

### Chore

 - <csr-id-ef5cd61f032d20ff42ea68caf22a4ac46355c137/> bump jax-service and jax-bucket to 0.1.2
 - <csr-id-d0a31f491f14927e4b5453daceeaafc963dd4171/> updated readme reference

### Chore

 - <csr-id-20eab70de45b734acd0e44f4340dcb6659b32e84/> update internal manifest versions

## v0.1.1 (2025-10-12)

<csr-id-20eab70de45b734acd0e44f4340dcb6659b32e84/>
<csr-id-d0a31f491f14927e4b5453daceeaafc963dd4171/>

### Chore

 - <csr-id-20eab70de45b734acd0e44f4340dcb6659b32e84/> update internal manifest versions
 - <csr-id-d0a31f491f14927e4b5453daceeaafc963dd4171/> updated readme reference

