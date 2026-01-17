# Project Layout

Exhaustive tree of the jax-bucket workspace.

## Crates

### app (`jax-bucket` binary)

CLI and daemon for bucket operations, HTTP API, and web UI.

### common (`jax-common` library)

Core library: crypto primitives, mount/manifest, peer protocol, blob storage.

---

## Tree

```
jax-bucket/
├── Cargo.toml                          # Workspace root
├── Cargo.lock
├── build.rs
├── release.toml                        # cargo-smart-release config
├── CLAUDE.md                           # Agent instructions
├── README.md
├── LICENSE
│
├── crates/
│   ├── app/
│   │   ├── Cargo.toml
│   │   ├── README.md                   # Crate README (crates.io)
│   │   ├── build.rs
│   │   ├── agents/                     # App-specific agent docs
│   │   │   ├── sqlite-sync-provider-example.md
│   │   │   └── templating/
│   │   │       ├── template-structure.md
│   │   │       └── templating-ui-system.md
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── args.rs
│   │   │   ├── op.rs
│   │   │   ├── state.rs
│   │   │   ├── daemon/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── config.rs
│   │   │   │   ├── state.rs
│   │   │   │   ├── sync_provider.rs
│   │   │   │   ├── database/
│   │   │   │   │   ├── mod.rs
│   │   │   │   │   ├── sqlite.rs
│   │   │   │   │   ├── bucket_queries.rs
│   │   │   │   │   ├── bucket_log_provider.rs
│   │   │   │   │   └── types/
│   │   │   │   │       ├── mod.rs
│   │   │   │   │       └── dcid.rs
│   │   │   │   ├── http_server/
│   │   │   │   │   ├── mod.rs
│   │   │   │   │   ├── config.rs
│   │   │   │   │   ├── api/
│   │   │   │   │   │   ├── mod.rs
│   │   │   │   │   │   ├── v0/
│   │   │   │   │   │   │   ├── mod.rs
│   │   │   │   │   │   │   └── bucket/
│   │   │   │   │   │   │       ├── mod.rs
│   │   │   │   │   │   │       ├── add.rs
│   │   │   │   │   │   │       ├── cat.rs
│   │   │   │   │   │   │       ├── create.rs
│   │   │   │   │   │   │       ├── delete.rs
│   │   │   │   │   │   │       ├── export.rs
│   │   │   │   │   │   │       ├── list.rs
│   │   │   │   │   │   │       ├── ls.rs
│   │   │   │   │   │   │       ├── mkdir.rs
│   │   │   │   │   │   │       ├── mv.rs
│   │   │   │   │   │   │       ├── ping.rs
│   │   │   │   │   │   │       ├── publish.rs
│   │   │   │   │   │   │       ├── rename.rs
│   │   │   │   │   │   │       ├── share.rs
│   │   │   │   │   │   │       └── update.rs
│   │   │   │   │   │   └── client/
│   │   │   │   │   │       ├── mod.rs
│   │   │   │   │   │       ├── client.rs
│   │   │   │   │   │       └── error.rs
│   │   │   │   │   ├── handlers/
│   │   │   │   │   │   ├── mod.rs
│   │   │   │   │   │   └── not_found.rs
│   │   │   │   │   ├── health/
│   │   │   │   │   │   ├── mod.rs
│   │   │   │   │   │   ├── data_source.rs
│   │   │   │   │   │   ├── liveness.rs
│   │   │   │   │   │   ├── readiness.rs
│   │   │   │   │   │   └── version.rs
│   │   │   │   │   └── html/
│   │   │   │   │       ├── mod.rs
│   │   │   │   │       ├── index.rs
│   │   │   │   │       ├── buckets/
│   │   │   │   │       │   ├── mod.rs
│   │   │   │   │       │   ├── file_editor.rs
│   │   │   │   │       │   ├── file_explorer.rs
│   │   │   │   │       │   ├── file_viewer.rs
│   │   │   │   │       │   ├── history.rs
│   │   │   │   │       │   └── peers.rs
│   │   │   │   │       └── gateway/
│   │   │   │   │           └── mod.rs
│   │   │   │   └── process/
│   │   │   │       ├── mod.rs
│   │   │   │       └── utils.rs
│   │   │   └── ops/
│   │   │       ├── mod.rs
│   │   │       ├── daemon.rs
│   │   │       ├── init.rs
│   │   │       ├── version.rs
│   │   │       └── bucket/
│   │   │           ├── mod.rs
│   │   │           ├── add.rs
│   │   │           ├── cat.rs
│   │   │           ├── clone.rs
│   │   │           ├── clone_state.rs
│   │   │           ├── create.rs
│   │   │           ├── list.rs
│   │   │           ├── ls.rs
│   │   │           ├── share.rs
│   │   │           └── sync.rs
│   │   ├── templates/
│   │   │   ├── layouts/
│   │   │   │   ├── base.html
│   │   │   │   └── explorer.html
│   │   │   ├── pages/
│   │   │   │   ├── index.html
│   │   │   │   ├── not_found.html
│   │   │   │   └── buckets/
│   │   │   │       ├── index.html
│   │   │   │       ├── editor.html
│   │   │   │       ├── viewer.html
│   │   │   │       ├── logs.html
│   │   │   │       ├── peers.html
│   │   │   │       ├── syncing.html
│   │   │   │       └── not_found.html
│   │   │   └── components/
│   │   │       ├── banners/
│   │   │       │   └── historical.html
│   │   │       ├── cards/
│   │   │       │   └── bucket.html
│   │   │       ├── editors/
│   │   │       │   └── inline.html
│   │   │       ├── modals/
│   │   │       │   ├── manifest.html
│   │   │       │   ├── share.html
│   │   │       │   └── upload.html
│   │   │       └── sidebars/
│   │   │           └── bucket.html
│   │   └── static/                     # Static assets (CSS, JS)
│   │
│   └── common/
│       ├── Cargo.toml
│       ├── README.md                   # Crate README (crates.io)
│       ├── build.rs
│       ├── src/
│       │   ├── lib.rs
│       │   ├── version.rs
│       │   ├── bucket_log/
│       │   │   ├── mod.rs
│       │   │   ├── memory.rs
│       │   │   └── provider.rs
│       │   ├── crypto/
│       │   │   ├── mod.rs
│       │   │   ├── keys.rs
│       │   │   ├── secret.rs
│       │   │   └── secret_share.rs
│       │   ├── linked_data/
│       │   │   ├── mod.rs
│       │   │   ├── ipld.rs
│       │   │   └── link.rs
│       │   ├── mount/
│       │   │   ├── mod.rs
│       │   │   ├── manifest.rs
│       │   │   ├── node.rs
│       │   │   ├── mount_inner.rs
│       │   │   ├── principal.rs
│       │   │   ├── pins.rs
│       │   │   ├── path_ops.rs
│       │   │   └── maybe_mime.rs
│       │   └── peer/
│       │       ├── mod.rs
│       │       ├── blobs_store.rs
│       │       ├── jobs.rs
│       │       ├── peer_builder.rs
│       │       ├── peer_inner.rs
│       │       ├── protocol/
│       │       │   ├── mod.rs
│       │       │   ├── bidirectional.rs
│       │       │   └── messages/
│       │       │       ├── mod.rs
│       │       │       ├── macros.rs
│       │       │       └── ping.rs
│       │       └── sync/
│       │           ├── mod.rs
│       │           ├── download_pins.rs
│       │           ├── ping_peer.rs
│       │           └── sync_bucket.rs
│       └── tests/
│           ├── common/
│           │   └── mod.rs
│           ├── add.rs
│           ├── ls.rs
│           ├── mirror.rs
│           ├── mkdir.rs
│           ├── mv.rs
│           ├── ops_log.rs
│           ├── persistence.rs
│           └── rm.rs
│
├── agents/                             # Agent documentation
│   ├── INDEX.md
│   ├── CONCEPTS.md
│   ├── CONTRIBUTING.md
│   ├── ISSUES.md
│   ├── PROJECT_LAYOUT.md
│   ├── RELEASE.md
│   ├── RUST_PATTERNS.md
│   └── SUCCESS_CRITERIA.md
│
├── issues/                             # Issue tracking
│
├── bin/                                # Build scripts
│   ├── build.sh
│   ├── check.sh
│   ├── dev.sh
│   └── test.sh
│
└── .github/
    └── workflows/
        ├── ci.yml
        ├── release-pr.yml
        └── publish-crate.yml
```
