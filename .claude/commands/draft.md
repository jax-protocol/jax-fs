---
description: Push current branch and create a draft PR into main
allowed-tools:
  - Bash(git:*)
  - Bash(gh:*)
---

Create a draft pull request for the current branch targeting `main`.

## Steps

1. Get the current branch name via `git branch --show-current`

2. Check if the branch has an upstream via `git status -sb`
   - If no upstream, push to origin with `-u` flag: `git push -u origin <branch>`
   - If upstream exists but behind, push: `git push`

3. Gather context by collecting commits unique to this branch vs main:
   ```
   git log main..HEAD --oneline
   ```

4. Create a draft PR with `gh pr create`:
   - Use `--draft` flag
   - Use `--base main`
   - Title should be descriptive of what the branch accomplishes (not just the branch name)
   - Body should summarize the changes based on the commits

5. Return the PR URL to the user

Note: Do not use `--no-verify` when pushing. Let git hooks run.
