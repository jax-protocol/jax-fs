# Success Criteria

This document defines what "done" means for agent work. All criteria must be met before creating a PR.

## Golden Rule

**You are not allowed to finish in a state where CI is failing.**

---

## Required Checks

Before considering work complete, run from the project root:

```bash
cargo build      # Must compile without errors
cargo test       # All tests must pass
cargo clippy     # No warnings
cargo fmt --check  # Code must be formatted
```

---

## CI Pipeline

GitHub Actions CI runs automatically on every push and PR.

### Checks That Run

| Check | Command | What It Verifies |
|-------|---------|------------------|
| Build | `cargo build` | Code compiles |
| Tests | `cargo test` | All tests pass |
| Clippy | `cargo clippy` | No lint warnings |
| Format | `cargo fmt --check` | Code is formatted |

**All checks must pass before merging.**

---

## Testing Your Changes

### Unit Tests

Unit tests live alongside the code in `#[cfg(test)]` modules:

```bash
# Run all tests
cargo test

# Run tests for specific crate
cargo test -p jax-common
cargo test -p jax-bucket

# Run specific test
cargo test test_mirror_cannot_mount
```

### Integration Tests

Integration tests live in `crates/*/tests/`:

```bash
# Run integration tests for common crate
cargo test -p jax-common --test mount_tests
```

---

## Fixing Common Issues

### Compile Errors

```bash
cargo build 2>&1 | head -50  # See first errors
# Fix errors, then rebuild
```

### Clippy Warnings

```bash
cargo clippy                 # See warnings
cargo clippy --fix           # Auto-fix some issues
# Fix remaining warnings manually
```

### Format Issues

```bash
cargo fmt                    # Auto-fix formatting
git add .
git commit -m "chore: fix formatting"
```

### Test Failures

```bash
cargo test -- --nocapture    # See test output
cargo test test_name         # Run specific test
# Fix tests, then rerun
```

---

## Documentation Requirements

Agents are responsible for keeping documentation up to date. If your changes affect any of the following, update the relevant docs:

**Update `agents/` when:**
- Adding new patterns or conventions
- Changing project structure
- Adding new crates

**Update inline documentation when:**
- Adding new public functions or types
- Changing function signatures or behavior

**Documentation locations:**
- `agents/` - Agent and developer guidance
- Rustdoc comments - Public API documentation
- `issues/` - Feature planning and tracking

---

## Pre-Commit Checklist

- [ ] `cargo build` succeeds
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings
- [ ] `cargo fmt --check` passes
- [ ] Tests written for new functionality
- [ ] Documentation updated if patterns/structure changed
- [ ] No debug code left behind (println!, dbg!)
- [ ] Changes are committed with descriptive messages
- [ ] Branch is pushed to remote
