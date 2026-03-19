# Contributing

Guide for both human contributors and AI agents working on jax-bucket.

## For All Contributors

### Getting Started

1. Clone the repository
2. Install Rust via [rustup](https://rustup.rs/) (1.75+)
3. Build: `cargo build`
4. Test: `cargo test`
5. Run CLI: `cargo run --bin jax -- --help`

### Making Changes

1. Create a feature branch from `main`
2. Make your changes following the patterns in `docs/PATTERNS.md` and `agents/RUST_PATTERNS.md`
3. Run checks: `cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt -- --check`
4. Commit with a clear message following conventional commits
5. Open a pull request

### Commit Message Format

We use [conventional commits](https://www.conventionalcommits.org/) for semantic versioning:

| Prefix | Use For | Version Bump |
|--------|---------|--------------|
| `feat:` | New features | Minor |
| `fix:` | Bug fixes | Patch |
| `feat!:` / `fix!:` | Breaking changes | Major |
| `refactor:` | Code refactoring | None |
| `chore:` | Maintenance | None |
| `docs:` | Documentation | None |
| `test:` | Tests | None |

Include scope when relevant: `feat(mount):`, `fix(fuse):`, `refactor(crypto):`

Example:
```
feat: add mirror principal role and bucket publishing workflow

- Implement PrincipalRole::Mirror for read-only peers
- Add publish/unpublish methods to Manifest
- Add integration tests for mirror mounting
```

## For AI Agents

### Context to Gather First

Before making changes, read:
- `CLAUDE.md` — Project overview and quick commands
- `docs/PATTERNS.md` — Coding conventions
- `docs/SUCCESS_CRITERIA.md` — CI checks that must pass
- `agents/RUST_PATTERNS.md` — Detailed Rust patterns
- `agents/CLI.md` — If touching CLI commands
- Related code files to understand existing patterns

### Workflow

1. **Understand** — Read the task and relevant code
2. **Plan** — Break down into small steps
3. **Implement** — Follow existing patterns
4. **Verify** — Run tests and checks
5. **Commit** — Clear, atomic commits

### Constraints

- Don't modify CI/CD configuration without approval
- Don't add new dependencies without discussion
- Don't refactor unrelated code
- Don't skip tests or use `--no-verify`
- Don't use `#[allow(dead_code)]` — remove unused code
- Don't print from `Op::execute()` — return typed data

### Test Readability

Tests must read like plain English — this is critical for AI-generated code verification:

- **Named actors**: Alice, Bob, Carol (not peer1, peer2)
- **Scenario names**: `scenario_alice_and_bob_both_create_notes_txt`
- **Section comments**: clear delineation of setup, action, verification
- **Helper structs**: DRY setup with `setup_test_env()`

## Code Review

- All PRs require CI to pass before merge
- Squash merge to main
- Review checklist in `agents/CONTRIBUTING.md`

## Pull Request Process

1. Create a branch with a descriptive name
2. Make changes, write tests
3. Run all checks: `cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt -- --check`
4. Push and create PR with clear title and summary
5. Wait for CI
6. Address feedback
7. Squash merge to main
