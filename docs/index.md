# Documentation Index

Central hub for project documentation. AI agents should read this first.

## Quick Start

```bash
# Build and verify
cargo build
cargo test
cargo clippy -- -D warnings
cargo fmt -- --check

# Run the CLI
cargo run --bin jax -- --help

# Start 2-node dev environment (requires tmux)
make dev
```

## Documentation

| Document | Purpose |
|----------|---------|
| [PATTERNS.md](./PATTERNS.md) | Coding conventions and patterns |
| [CONTRIBUTING.md](./CONTRIBUTING.md) | How to contribute (agents + humans) |
| [SUCCESS_CRITERIA.md](./SUCCESS_CRITERIA.md) | CI checks that must pass |

### Detailed Guides (agents/)

| Document | Purpose |
|----------|---------|
| `agents/PROJECT_LAYOUT.md` | Crate structure, modules, key files |
| `agents/RUST_PATTERNS.md` | Error handling, async, serialization, module org |
| `agents/CLI.md` | Op pattern, formatting boundary, command_enum! |
| `agents/CONTRIBUTING.md` | Test readability, commit conventions, review checklist |
| `agents/DEVELOPMENT.md` | Dev environment, tmux setup, debugging |
| `agents/API.md` | HTTP API reference |

## For AI Agents

You are an autonomous coding agent working on a focused task.

### Workflow

1. **Understand** — Read the task description and relevant docs
2. **Explore** — Search the codebase to understand context
3. **Plan** — Break down work into small steps
4. **Implement** — Follow existing patterns in `PATTERNS.md` and `agents/RUST_PATTERNS.md`
5. **Verify** — Run checks from `SUCCESS_CRITERIA.md`
6. **Commit** — Clear, atomic commits using conventional commit format

### Guidelines

- Follow existing code patterns and conventions
- Make atomic commits (one logical change per commit)
- Add tests for new functionality — tests must read like stories (named actors, scenario names)
- Update documentation if behavior changes
- If blocked, commit what you have and note the blocker
- CLI commands use the Op pattern — never print from execute(), return typed data
- Use `thiserror` for error types, `?` for propagation, `#[from]` for conversion
- Use `tokio` for all async, `#[tokio::test]` for async tests

### When Complete

Your work will be reviewed and merged by the parent session.
Ensure all checks pass before finishing.
