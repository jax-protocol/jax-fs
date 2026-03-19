---
description: Review branch changes against project conventions. Use when preparing to merge, checking code quality, or validating changes before PR.
allowed-tools:
  - Bash(git diff:*)
  - Bash(git log:*)
  - Bash(git status)
  - Bash(git branch:*)
  - Read
  - Glob
  - Grep
---

Review the current branch's changes against jax-bucket conventions before merge.

## Steps

### 1. Gather Context

Read project conventions:
- `CLAUDE.md` — project guide and constraints
- `docs/PATTERNS.md` — error handling, module org, method ordering, naming
- `docs/CLI.md` — Op pattern, formatting boundary, ui module
- `docs/CONTRIBUTING.md` — test readability, commit conventions, review checklist

### 2. Collect Changes

Get the full picture of what this branch changes:
```
git log main..HEAD --oneline
git diff main...HEAD --stat
git diff main...HEAD
```
If `main` doesn't exist, try `origin/main`.

### 3. Commit Message Audit

Check each commit message:
```
git log main..HEAD --format="%h %s"
```
Verify they use conventional commits (`feat:`, `fix:`, `refactor:`, etc.) with clear descriptions.

### 4. Code Review

Review the diff for:
- **Correctness**: Does the logic do what the commit messages claim?
- **Patterns**: Uses `thiserror` for errors, `?` for propagation, `#[from]` for conversion?
- **Op pattern**: CLI commands return typed data, never print? Display impls use `ui::` helpers?
- **Module org**: Methods ordered as constructors → getters → setters? Files focused (< 200 lines)?
- **Naming**: `is_*`/`can_*`/`has_*` for predicates? Descriptive over short?
- **No dead code**: All public methods have callers? No `#[allow(dead_code)]`?
- **Error handling**: Appropriate for context? No unwrap in library code?
- **Security**: No credentials, injection risks, or unsafe operations?
- **Tests**: Readable (named actors, scenario names, section comments)? Changes covered?
- **Dead code**: No debug code (`println!`, `dbg!`), commented-out blocks, unused imports?

### 5. Documentation Check

- `docs/PROJECT_LAYOUT.md` — new files/modules added?
- `docs/PATTERNS.md` — new patterns introduced?
- `CLAUDE.md` — project structure or constraints changed?
- Issue tickets — status updates needed?

### 6. Skills Check

If behavior changed that affects skills in `.claude/skills/`:
- `/check` — Did build, test, or lint commands change?
- `/review` — Did review criteria or conventions change?

### 7. Issue Cross-Reference

If `issues/` exists, check for related tickets — should any status be updated?

## Output Format

```
## Commit Messages
- [PASS/FAIL] Format and clarity
- Issues: (list or "None")

## Code Review
- [PASS/WARN/FAIL] Correctness
- [PASS/WARN/FAIL] Conventions (patterns, naming, module org)
- [PASS/WARN/FAIL] Op pattern compliance
- [PASS/WARN/FAIL] Error handling
- [PASS/WARN/FAIL] Security
- [PASS/WARN/FAIL] Test coverage and readability
- Suggestions: (list or "None")

## Documentation
- [PASS/WARN] Updates needed: (list or "None")

## Skills
- [PASS/WARN] Updates needed: (list or "None")

## Summary
[Overall assessment and recommended actions before merge]
```

Be specific — reference file paths and line numbers where relevant.
