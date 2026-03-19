# Claude Code Instructions for jax-bucket

## Project Overview

jax-bucket is a P2P encrypted storage system built in Rust. It uses content-addressed blob storage via iroh-blobs, ChaCha20-Poly1305 encryption, and X25519 secret sharing for access control.

## Quick Start

```bash
cargo build      # Build all crates
cargo test       # Run all tests
cargo clippy     # Check for lint warnings
cargo fmt        # Format code
```

## Project Structure

- `crates/daemon/` - Main binary (`jax-daemon`) with CLI and daemon
- `crates/desktop/` - Tauri desktop app (`jax-desktop`)
- `crates/common/` - Shared library (`jax-common`) with crypto, mount, and peer modules
- `agents/` - Agent documentation (read these first)
- `issues/` - Issue tracking (epics and tickets)

## Key Documentation

Before starting work, read the relevant docs in `agents/`:

- `CONCEPTS.md` - High-level architecture and key concepts
- `PROJECT_LAYOUT.md` - Crate structure and modules
- `RUST_PATTERNS.md` - Error handling, async patterns, serialization
- `SUCCESS_CRITERIA.md` - CI requirements (must pass before PR)

## Constraints

1. **All CI checks must pass** before creating a PR:
   - `cargo build` - Must compile
   - `cargo test` - All tests pass
   - `cargo clippy` - No warnings
   - `cargo fmt --check` - Code formatted

2. **Follow existing patterns** - Match the style of existing code

3. **Write tests** - Unit tests in `#[cfg(test)]` modules, integration tests in `tests/`

4. **Update relevant docs** - Keep `agents/` docs in sync with code changes

## Do Not

- Push to main directly - create a PR
- Skip clippy warnings - fix them
- Add debug code (println!, dbg!) to commits
- Create documentation files unless explicitly asked
