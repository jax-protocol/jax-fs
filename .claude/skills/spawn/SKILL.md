---
description: Spawn parallel Claude Code workers for task execution. Use to parallelize work across multiple worktrees.
allowed-tools:
  - Bash(jig spawn:*)
  - Bash(jig ps)
  - Bash(jig attach:*)
  - Bash(jig review:*)
  - Bash(jig merge:*)
  - Bash(jig kill:*)
  - Bash(git status)
  - Bash(git log:*)
  - Bash(git diff:*)
  - Bash(git branch:*)
  - Read
  - Glob
  - Grep
---

Spawn parallel Claude Code workers to execute a set of tasks.

## Prerequisites

Before spawning, you should have a clear picture of the work. Run `/issues` first to discover and understand what needs to be done.

## Workflow

1. **Decompose** the work into independent, parallelizable tasks
2. **Spawn** a worker for each task:
   ```bash
   jig spawn <task-name> --context "<detailed context>" --auto
   ```
3. **Monitor** progress:
   ```bash
   jig ps
   ```
4. **Review** completed work:
   ```bash
   jig review <task-name>
   ```
5. **Merge** approved work into the current branch:
   ```bash
   jig merge <task-name>
   ```

## Writing Good Context

Each `--context` value is the worker's entire prompt. Include:
- **One-line summary** of what to accomplish
- **Files to modify** (if known)
- **Specific requirements** and constraints
- **Acceptance criteria** — how do we know it's done?
- **What NOT to do** — boundaries to prevent scope creep

Example:
```bash
jig spawn add-auth-middleware --context "Add JWT auth middleware to the Express API.
Modify src/middleware/auth.ts. Use jsonwebtoken package (already installed).
Protect all routes under /api/v1/ except /api/v1/health.
Return 401 with {error: 'unauthorized'} on invalid/missing token.
Do NOT modify the health endpoint or add any new packages." --auto
```

## Rules

- Keep tasks independent — workers cannot see each other's changes
- Include enough context for the worker to be fully autonomous
- Spawn 2-4 workers at a time to avoid resource contention
- Always review before merging (`jig review <name>`)
- Use `jig kill <name>` if a worker is stuck or going off track
- After merging, check for conflicts before spawning the next batch
