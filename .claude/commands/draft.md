---
description: Push current branch and create a draft PR into main
allowed-tools:
  - Bash(git *)
  - Bash(gh pr *)
  - Bash(cargo clippy *)
  - Bash(cargo fmt *)
---

Create a draft pull request for the current branch targeting `main`.

## Steps

1. Get the current branch name via `git branch --show-current`

2. Check for uncommitted changes via `git status --porcelain`
   - If there are uncommitted changes (modified, added, or untracked files):
     a. Run `cargo fmt` to format code
     b. Run `cargo clippy` to check for warnings (fix any issues)
     c. Stage ALL changes: `git add -A`
     d. Create a commit with a descriptive message based on the changes
     e. Use conventional commit format (feat:, fix:, docs:, etc.)

3. Check if the branch has an upstream via `git status -sb`
   - If no upstream, push to origin with `-u` flag: `git push -u origin <branch>`
   - If upstream exists, push: `git push`

4. Gather context by collecting commits unique to this branch vs main:
   ```
   git log main..HEAD --oneline
   ```

5. Create a draft PR with `gh pr create`:
   - Use `--draft` flag
   - Use `--base main`
   - Title should be descriptive of what the branch accomplishes
   - Body should summarize ALL the changes based on the commits

6. Return the PR URL to the user

## Important

- **Commit ALL uncommitted changes** before pushing - don't leave anything behind
- Do not use `--no-verify` when pushing - let git hooks run
- If clippy has warnings, fix them before committing
