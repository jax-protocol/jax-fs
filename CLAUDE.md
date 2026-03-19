# Project Guide

jax-bucket: end-to-end encrypted, peer-to-peer storage built on iroh-blobs with ChaCha20-Poly1305 encryption and X25519 secret sharing.

## Quick Reference

```bash
cargo build                # Build all crates
cargo test                 # Run all tests
cargo clippy -- -D warnings # Lint (warnings are errors)
cargo fmt                  # Format code
cargo fmt -- --check       # Check formatting
make dev                   # Start 2-node dev environment in tmux
cargo run --bin jax -- --help # Run the CLI
```

## Project Structure

```
crates/
├── daemon/       # CLI binary + daemon library (jax-daemon)
│   ├── src/cli/  # CLI commands using the Op pattern (see agents/CLI.md)
│   ├── src/http_server/ # Axum REST API + gateway
│   ├── src/fuse/  # FUSE filesystem (feature-gated)
│   └── src/database/ # SQLite persistence
├── common/       # Core library (jax-common)
│   ├── src/crypto/ # Ed25519/X25519 keys, ChaCha20-Poly1305, secret sharing
│   ├── src/mount/  # Virtual filesystem, manifest, CRDT path ops
│   └── src/peer/   # P2P via iroh, blob storage, sync protocol
├── object-store/ # Blob storage backend (SQLite + S3/MinIO/local)
└── desktop/      # Tauri 2.0 desktop app (SolidJS frontend)

agents/           # Detailed architecture docs (read these first)
bin/              # Dev scripts (dev, check, db, minio)
issues/           # File-based issue tracking
```

## Documentation

- `agents/CONCEPTS.md` — High-level architecture and key concepts
- `agents/PROJECT_LAYOUT.md` — Crate structure and module map
- `agents/RUST_PATTERNS.md` — Error handling, async, serialization, module org
- `agents/CLI.md` — Op pattern, formatting boundary, command_enum! macro
- `agents/CONTRIBUTING.md` — Contribution workflow, commit conventions, test readability
- `agents/DEVELOPMENT.md` — Dev environment setup, 2-node tmux workflow
- `agents/API.md` — HTTP API reference
- `docs/index.md` — Documentation hub and agent instructions
- `docs/PATTERNS.md` — Coding conventions
- `docs/SUCCESS_CRITERIA.md` — CI checks
- `docs/CONTRIBUTING.md` — Contribution guide

## Issues

Track work items in `issues/`. See `issues/README.md` for the convention.

## Constraints

1. **All CI checks must pass** before creating a PR:
   - `cargo build` — must compile
   - `cargo test` — all tests pass
   - `cargo clippy -- -D warnings` — no warnings
   - `cargo fmt -- --check` — code formatted
2. **Follow existing patterns** — match style of existing code (see `agents/RUST_PATTERNS.md`)
3. **Write tests** — unit tests in `#[cfg(test)]` modules, integration tests in `tests/`
4. **Tests must read like stories** — named actors (Alice, Bob), scenario-based names, clear section comments
5. **Update docs** — keep `agents/` in sync with code changes
6. **Op pattern for CLI** — commands return typed data, never print; formatting in Display impls
7. **Module per responsibility** — split files > 200 lines with distinct sections

## Do Not

- Push to main directly — create a PR
- Skip clippy warnings — fix them
- Add debug code (`println!`, `dbg!`) to commits
- Use `#[allow(dead_code)]` — remove unused code instead
- Write speculative code without a caller
- Print from Op::execute() — return data, format in Display
- Put infrastructure config as daemon flags — set at init time
- Add `--json` flag to CLI — use the HTTP API for machine output
- Create documentation files unless explicitly asked
