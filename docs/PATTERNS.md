# Coding Patterns

Conventions for jax-bucket. See `agents/RUST_PATTERNS.md` for detailed examples.

## Error Handling

- Define per-module error types with `thiserror::Error`
- Use `#[from]` for automatic conversion between error types
- Propagate with `?` operator
- Library code (`jax-common`): specific typed errors
- Application code (`jax-daemon`): can use `anyhow` at the top level
- Never `unwrap()` in library code

```rust
#[derive(Debug, thiserror::Error)]
pub enum MountError {
    #[error("path not found: {0}")]
    PathNotFound(String),

    #[error("crypto error: {0}")]
    Crypto(#[from] CryptoError),
}
```

## Module Organization

Standard module structure:

```
module_name/
├── mod.rs           # Public exports (pub use)
├── types.rs         # Type definitions
├── error.rs         # Error types
└── impl.rs          # Implementation
```

Simpler modules go in a single file with sections: Types, Error, Implementation, Tests.

Public API via `mod.rs` with selective re-exports:

```rust
pub use manifest::{Manifest, Share};
pub use mount_inner::{Mount, MountError};
```

Keep files focused — one responsibility per file. Split when > 200 lines with distinct sections.

## Naming Conventions

- **Types**: `PascalCase` — `Mount`, `MountError`, `PublicKey`
- **Functions/methods**: `snake_case` — `from_hex()`, `add_owner()`
- **Constants**: `UPPER_SNAKE_CASE` — `PRIVATE_KEY_SIZE`, `API_PREFIX`
- **Modules/files**: `snake_case` — `mount_inner.rs`, `blobs_store.rs`
- **Predicates**: `is_*` (state), `can_*` (capability), `has_*` (presence)

### Method Ordering

Organize `impl` blocks:

1. Constructors (`new`, `new_*`, `with_*`, `from_*`)
2. Getters (prefixed with `/* Getters */` comment)
3. Setters/Mutators (prefixed with `/* Setters */` comment)

### Naming Philosophy

Prefer descriptive names over short ones. Type names are nouns, function names are verbs.

## Output Conventions

- **stdout**: machine-readable output via `println!("{output}")` in `main.rs`
- **stderr**: errors via `eprintln!()` at the formatting boundary
- **Ops never print** — they return typed data; `Display` impls handle presentation
- **Progress bars**: via `indicatif::MultiProgress` on `OpContext`, hidden when not TTY
- **UI helpers**: `cli/ui.rs` — status symbols, colors, tables, truncation
- **`--plain` flag**: disables colors and table borders globally
- **No `--json` flag** — use the HTTP API for machine output

## Testing Patterns

### Unit tests in same file

```rust
#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_share_new_mirror() {
        let key = SecretKey::generate();
        let share = Share::new_mirror(key.public());
        assert!(share.is_mirror());
    }
}
```

### Integration tests in `crates/*/tests/`

Separate files for cross-module tests. Use `#[tokio::test]` for async.

### Test readability requirements

- **Named actors**: Alice, Bob, Carol — not "peer1", "peer2"
- **Scenario-based names**: `scenario_alice_and_bob_both_create_notes_txt`
- **Section comments**: clear delineation of setup, actions, verification
- **Helper structs**: `setup_test_env()` for DRY setup, scenario-specific helpers when needed

### Serialization

- Use `serde` with IPLD DAG-CBOR (`serde_ipld_dagcbor`) for content-addressed storage
- `#[serde(skip_serializing_if = "Option::is_none")]` for optional fields
- All persistent data structures derive `Serialize, Deserialize`

## Common Idioms

### The Op Pattern (CLI)

Every CLI command implements `Op` trait — receives `OpContext`, returns typed `Output` or `Error`. The `command_enum!` macro generates dispatch. See `agents/CLI.md` for details.

### Async

- All async code uses `tokio`
- `Arc<tokio::sync::Mutex<T>>` for shared mutable state
- `flume` channels for multi-producer, multi-consumer
- `parking_lot` for blocking operations

### Content Addressing

Data is serialized to DAG-CBOR, stored as content-addressed blobs via iroh-blobs, and referenced by `Link` (CID wrapper).
