# Success Criteria

Checks that must pass before code can be merged. This is the CI gate.

**Golden Rule: You are not allowed to finish in a state where CI is failing.**

## Quick Check

```bash
cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt -- --check
```

Or use Make targets:

```bash
make build && make test && make lint && make fmt-check
```

## Individual Checks

### Build

```bash
cargo build
# or: make build (runs cargo build --all)
```

### Tests

```bash
cargo test
# or: make test (runs cargo test --all)

# Per-crate:
cargo test -p jax-common
cargo test -p jax-daemon

# Specific test:
cargo test test_mirror_cannot_mount

# With output:
cargo test -- --nocapture
```

### Linting

```bash
cargo clippy -- -D warnings
# or: make lint (runs cargo clippy --all -- -D warnings)

# Auto-fix some issues:
cargo clippy --fix
```

### Formatting

```bash
cargo fmt -- --check    # Check
cargo fmt               # Fix
# or: make fmt-check / make fmt
```

### Type Checking

Handled by the Rust compiler via `cargo build` and `cargo check`.

## Fixing Common Issues

### Formatting Failures

```bash
cargo fmt
```

### Lint Warnings

```bash
cargo clippy --fix              # Auto-fix what's possible
cargo clippy -- -D warnings     # See remaining issues
# Fix remaining warnings manually
```

### Test Failures

```bash
cargo test -- --nocapture       # See test output
cargo test test_name            # Run specific test
RUST_LOG=debug cargo test       # With debug logging
```

### Compile Errors

```bash
cargo build 2>&1 | head -50    # See first errors
```

## Pre-Commit Checklist

- [ ] `cargo build` succeeds
- [ ] `cargo test` passes
- [ ] `cargo clippy -- -D warnings` has no warnings
- [ ] `cargo fmt -- --check` passes
- [ ] Tests written for new functionality
- [ ] No debug code left behind (`println!`, `dbg!`)
- [ ] Documentation updated if patterns/structure changed
- [ ] Changes committed with descriptive conventional commit messages
