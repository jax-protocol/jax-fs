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

### Detailed Guides

| Document | Purpose |
|----------|---------|
| [PROJECT_LAYOUT.md](./PROJECT_LAYOUT.md) | Crate structure, modules, key files |
| [CLI.md](./CLI.md) | Op pattern, formatting boundary, command_enum! |
| [DEVELOPMENT.md](./DEVELOPMENT.md) | Dev environment, tmux setup, debugging |
| [DEBUG.md](./DEBUG.md) | Debugging workflow, log inspection, API testing |
| [API.md](./API.md) | HTTP API reference |
| [INSTALL.md](./INSTALL.md) | Installation and setup guide |
| [RELEASE.md](./RELEASE.md) | Release process and automation |
| [ISSUES.md](./ISSUES.md) | Issue and ticket conventions |
| [concepts/](./concepts/) | Architecture: overview, data model, crypto, sync, security |

## For AI Agents

You are an autonomous coding agent working on a focused task.

### Workflow

1. **Understand** — Read the task description and relevant docs
2. **Explore** — Search the codebase to understand context
3. **Plan** — Break down work into small steps
4. **Implement** — Follow existing patterns in [PATTERNS.md](./PATTERNS.md)
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
