# Contributing Guide

This guide covers how to contribute to jax-bucket, whether you're an AI agent or a human developer.

## For AI Agents

### Getting Started

1. **Run `cargo build`** - Verify the project compiles
2. **Read the relevant docs** - Start with [PROJECT_LAYOUT.md](./PROJECT_LAYOUT.md) and [PATTERNS.md](./PATTERNS.md)
3. **Understand the task** - Use planning mode to analyze requirements before coding
4. **Follow existing patterns** - Match the style and structure of existing code

### Key Constraints

- **Use worktrees for parallel work** - Use `git worktree` when working on multiple features
- **All tests must pass** - Run `cargo test` before submitting
- **Clippy must be clean** - Run `cargo clippy` and fix all warnings
- **Follow Rust idioms** - Use `?` for error propagation, prefer iterators over loops

### Code Quality Expectations

- Follow [PATTERNS.md](./PATTERNS.md) for Rust code
- Write tests for new functionality (see **Test Readability** below)
- Keep functions focused - single responsibility
- Document public APIs with rustdoc comments

### Test Readability (Critical)

**Tests must read like plain English.** This is not optional - the codebase relies heavily on AI-generated code, and expressive tests are the primary way to verify correctness without reading every line.

#### Structure tests as stories

Each test should tell a story with:
- **WHO**: Named actors (Alice, Bob, Carol - not "peer1", "peer2")
- **WHAT**: Actions in plain English (`.creates_file()`, `.saves()`)
- **WHEN**: Clear timeline/sequence
- **OUTCOME**: Explicit verification with descriptive assertions

#### Use descriptive variable names

```rust
// Good - named actors
let (mut alice, blobs, _, _temp) = setup_test_env().await;
let (mut bob, bob_key) = fork_mount(&mut alice, &blobs).await;

// Bad - generic names
let (mut mount1, blobs, _, _temp) = setup_test_env().await;
let (mut mount2, key2) = fork_mount(&mut mount1, &blobs).await;
```

#### Write scenario-based test names

```rust
// Good - describes the scenario
#[test]
async fn scenario_alice_and_bob_both_create_notes_txt() { ... }

#[test]
async fn scenario_alice_and_bob_diverge_for_three_versions() { ... }

// Bad - describes implementation
#[test]
async fn test_merge_with_conflict_file_resolver() { ... }
```

#### Use clear section comments

```rust
// =========================================================================
// ALICE'S CHAIN: Three versions of changes
// =========================================================================

// Version 1: Alice sets up the project
alice.creates_file("/README.md", "# My Project").await;
alice.saves(&blobs).await;

// =========================================================================
// VERIFY: All files from both chains exist
// =========================================================================
```

#### Create test-specific helper structs

When the standard `setup_test_env()` doesn't fit, create scenario-specific helpers:

```rust
struct TestScenario {
    blobs: BlobsStore,
    _temp: TempDir,
}

impl TestScenario {
    async fn create_peer(&self, name: &'static str) -> Peer { ... }
    async fn fork_peer(&self, original: &mut Peer, name: &'static str) -> Peer { ... }
}
```

#### Example: Good integration test

See `crates/common/tests/conflict_resolution.rs` for the canonical example of readable tests.

### File Naming Conventions

- Use `snake_case` for all file names (standard Rust convention)
- Example: `mount_inner.rs`, `secret_share.rs`, `blobs_store.rs`
- Module files use `mod.rs` or the module name directly

### Naming Philosophy

**Prefer descriptive names over short ones.** Clarity is more important than brevity.

- Function/file names should describe what they do
- Don't abbreviate unless universally understood
- Type names should be nouns, function names should be verbs

**Examples:**
```rust
// Good - descriptive
pub async fn add_owner(&mut self, peer: PublicKey) -> Result<(), MountError>
pub async fn add_mirror(&mut self, peer: PublicKey)
pub fn is_published(&self) -> bool
pub struct MirrorCannotMount;

// Bad - too short or ambiguous
pub async fn add(&mut self, p: PublicKey)
pub fn published(&self) -> bool
pub struct MirrorError;
```

### Before Submitting

1. Run `cargo build` - Must compile without errors
2. Run `cargo test` - All tests must pass
3. Run `cargo clippy` - Fix all warnings
4. Run `cargo fmt` - Code must be formatted
5. Write descriptive commit messages
6. Create PR with clear summary

---

## For Human Developers

### Development Setup

1. **Clone the repository**
   ```bash
   git clone git@github.com:jax-protocol/jax-buckets.git
   cd jax-buckets
   ```

2. **Install Rust** (if not already installed)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

3. **Build the project**
   ```bash
   cargo build
   ```

4. **Run tests**
   ```bash
   cargo test
   ```

### Parallel Development with Worktrees

For working on multiple lifts (features/fixes) in parallel, use git worktrees:

```bash
git worktree add ../my-feature feature/my-feature
```

Each worktree is an isolated working directory with its own branch.

---

## Commit Conventions

We use [conventional commits](https://www.conventionalcommits.org/) to automate semantic versioning and changelog generation.

### Prefixes and Semver Impact

| Prefix | Use For | Version Bump |
|--------|---------|--------------|
| `feat:` | New features | Minor (0.1.0 → 0.2.0) |
| `fix:` | Bug fixes | Patch (0.1.0 → 0.1.1) |
| `feat!:` | Breaking feature | Major (0.1.0 → 1.0.0) |
| `fix!:` | Breaking fix | Major (0.1.0 → 1.0.0) |
| `refactor:` | Code refactoring | None |
| `chore:` | Maintenance | None |
| `docs:` | Documentation | None |
| `test:` | Tests | None |
| `perf:` | Performance | None |

Breaking changes can also be indicated in the commit body:
```
feat: redesign sync protocol

BREAKING CHANGE: sync protocol v2 is incompatible with v1
```

### How Commits Drive Releases

`cargo-smart-release` scans commit messages since the last tag to determine:
1. **What version to bump** - Based on `feat:`, `fix:`, and breaking changes
2. **What goes in the changelog** - All conventional commits are categorized

This means your commit messages directly control the release process. See [RELEASE.md](./RELEASE.md) for full details.

### Example

```
feat: add mirror principal role and bucket publishing workflow

- Implement PrincipalRole::Mirror for read-only peers
- Add publish/unpublish methods to Manifest
- Extend /share endpoint with role parameter
- Add integration tests for mirror mounting

Co-Authored-By: Claude <noreply@anthropic.com>
```

---

## Pull Request Process

1. **Create a branch** - Use descriptive names (e.g., `feature/mirror-publishing`)
2. **Make changes** - Follow patterns, write tests
3. **Run checks** - `cargo build && cargo test && cargo clippy`
4. **Push and create PR** - Use descriptive title and summary
5. **Wait for CI** - All checks must pass
6. **Address feedback** - Respond to review comments
7. **Merge** - Squash merge to main

---

## Code Review Checklist

Quick checks for PR review (see also [PATTERNS.md](./PATTERNS.md) for detailed patterns).

### API Surface
- [ ] CLI flags are minimal and well-designed
- [ ] Infrastructure config is at init time, not daemon time
- [ ] No unnecessary env var support
- [ ] Related params combined (URLs vs separate fields)

### Code Quality
- [ ] New code follows existing patterns (check similar modules)
- [ ] Files have single responsibility (< 200 lines typical)
- [ ] Setup logic in dedicated modules, not mixed with state
- [ ] All public methods have callers
- [ ] No `#[allow(dead_code)]` without justification
- [ ] No speculative abstractions (Deref, From, etc. without use)

### Documentation Sync
- [ ] PROJECT_LAYOUT.md updated for new files/modules
- [ ] Issue tickets updated if implementation differs from spec
- [ ] Downstream tickets checked for outdated references

---

## Getting Help

- **Documentation issues** - Update the relevant doc and submit a PR
- **Bug reports** - Open a GitHub issue with reproduction steps
- **Feature requests** - Open a GitHub issue with use case description
