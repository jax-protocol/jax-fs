---
description: Review branch changes against project conventions
allowed-tools:
  - Bash(git:*)
  - Bash(ls:*)
  - Read
  - Glob
  - Grep
  - mcp__conductor__GetWorkspaceDiff
---

Review the current branch changes against project conventions defined in `agents/RUST_PATTERNS.md` and `CONTRIBUTING.md`.

## Context Files

Read these files first to understand conventions:
- `agents/RUST_PATTERNS.md` - Rust code patterns (error handling, async, serialization)
- `CONTRIBUTING.md` - Contribution guidelines (conventional commits, PR format)
- `agents/PROJECT_LAYOUT.md` - Current project structure

## Review Steps

### 1. Commit Message Audit

Check commits on this branch vs main:
```
git log main..HEAD --format="%s"
```

Verify each commit follows conventional commit format from CONTRIBUTING.md:
- `feat:` / `fix:` / `docs:` / `refactor:` / `test:` / `chore:` / `perf:`
- Breaking changes use exclamation mark or BREAKING CHANGE in body
- Messages are clear and descriptive

Report any commits that need amending with suggested corrections.

### 2. Code Pattern Review

Use the workspace diff to review changed Rust code against RUST_PATTERNS.md:

- **Error handling**: Uses `thiserror` in library code, proper error propagation with `?`
- **Async patterns**: Uses `tokio`, proper `#[tokio::test]` for async tests
- **Serialization**: Uses `serde` with DAG-CBOR where appropriate
- **Module organization**: Follows standard structure (mod.rs exports, etc.)
- **Type patterns**: Builder pattern, predicate methods (`is_*`, `can_*`)
- **Testing**: Unit tests in same file, integration tests in `tests/`

### 3. Documentation Audit

Check if documentation needs updates:

1. **PROJECT_LAYOUT.md**: Does the tree match actual files?
   - Check for new/removed files not reflected in the tree
   - Verify crate descriptions are accurate

2. **README files**: Do crate READMEs need updates?
   - `crates/app/README.md` - CLI changes?
   - `crates/common/README.md` - API changes?

3. **CONCEPTS.md**: Do architectural changes need documentation?

### 4. Issue Cross-Reference

Check `issues/` directory for related tickets:
```
ls issues/
```

For each issue file, check if this branch addresses or impacts it:
- Should any issue status be updated?
- Should new issues be created for follow-up work?
- Are there epic tickets that need progress updates?

## Output Format

Provide a structured review report:

```
## Commit Messages
- [PASS/FAIL] Conventional commit format
- Commits needing amendment: (list or "None")

## Code Patterns
- [PASS/WARN/FAIL] Error handling
- [PASS/WARN/FAIL] Async patterns
- [PASS/WARN/FAIL] Module organization
- Suggestions: (list or "None")

## Documentation
- [PASS/WARN] PROJECT_LAYOUT.md up to date
- [PASS/WARN] README updates needed
- [PASS/WARN] CONCEPTS.md updates needed
- Action items: (list or "None")

## Related Issues
- Issues addressed by this branch: (list or "None")
- Issues needing status update: (list or "None")
- Suggested follow-up issues: (list or "None")

## Summary
[Overall assessment and recommended actions before merge]
```

Be specific about what needs to change and why. Reference line numbers and file paths where relevant.
