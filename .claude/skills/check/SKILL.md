---
description: Run project checks (build, test, lint, format). Use when validating code quality, preparing for merge, or verifying changes pass CI.
allowed-tools:
  - Bash(cargo:*)
  - Bash(make:*)
  - Bash(cat:*)
  - Bash(ls:*)
  - Read
  - Glob
  - Grep
---

Run the full success criteria checks for jax-bucket.

## Steps

1. Run all four checks sequentially from the project root. Stop on first failure:

   ```bash
   cargo build
   ```

   ```bash
   cargo test
   ```

   ```bash
   cargo clippy -- -D warnings
   ```

   ```bash
   cargo fmt -- --check
   ```

2. If formatting fails, auto-fix and report:
   ```bash
   cargo fmt
   ```

3. If clippy fails, try auto-fix:
   ```bash
   cargo clippy --fix --allow-dirty
   ```

4. Report a summary:
   - Build: PASS/FAIL
   - Tests: PASS/FAIL
   - Clippy: PASS/FAIL
   - Format: PASS/FAIL (auto-fixed if applicable)

5. If any checks fail that cannot be auto-fixed, report what needs manual attention.

This is the gate for all PRs — all checks must pass before merge.
