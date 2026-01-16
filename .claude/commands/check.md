---
description: Run success criteria checks (build, test, clippy, fmt)
allowed-tools:
  - Bash(cargo:*)
---

Run the full success criteria checks to validate code quality before merging.

## Steps

1. Run `cargo build` to verify compilation

2. Run `cargo test` to execute all tests

3. Run `cargo clippy` to check for lint warnings

4. Run `cargo fmt --check` to verify formatting
   - If formatting check fails, run `cargo fmt` to auto-fix

5. Report a summary of pass/fail status for each check:
   - Build
   - Tests
   - Clippy
   - Format

6. If any checks fail that cannot be auto-fixed, report what needs manual attention.

This is the gate for all PRs - all checks must pass before merge.
