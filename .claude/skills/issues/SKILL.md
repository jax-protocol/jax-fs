---
description: Discover and manage work items. Use to explore tasks before spawning workers or to track project progress.
allowed-tools:
  - Bash(jig:*)
  - Bash(ls:*)
  - Bash(mkdir:*)
  - Read
  - Write
  - Edit
  - Glob
  - Grep
---

Discover and manage work items via `jig issues`. This is the first-class way to understand what work exists, what's in progress, and what to do next.

Issues are tracked in **Linear**. Use `jig issues` to query Linear tickets directly — no need to open the Linear UI. The CLI handles filtering, dependency resolution, and status transitions.

## Discovery workflow

Start here when picking up work or building context:

```bash
# See all active issues for the repo
jig issues

# High-priority items needing attention
jig issues --priority high
jig issues --priority urgent

# What's planned and ready to start (dependencies satisfied)
jig issues --unblocked --status planned

# What's currently blocked
jig issues --blocked

# Auto-spawn candidates (planned + labeled + deps satisfied)
jig issues --auto

# IDs only (for scripting / piping to spawn)
jig issues --ids --status planned
```

## Actions

### Show issue

```bash
jig issues <id>
# e.g. jig issues AUT-5044
```

### Filter by category, label, status

```bash
# By project/category
jig issues --category Engineering

# By label (all must match)
jig issues --label backend --label auto

# Combine filters
jig issues --status planned --priority high --label auto
```

### Update status

```bash
jig issues status <id> --status in-progress
jig issues status <id> --status blocked
```

### Complete issue

```bash
jig issues complete <id>
```

### Stats

```bash
jig issues stats
jig issues stats -g   # across all tracked repos
```

### Global scope

```bash
# Any command works across all repos with -g
jig issues -g
jig issues -g --status in-progress
```

## Dependencies

Issues can depend on other issues via Linear's `is_blocked_by` relations. Dependencies must be `Complete` before the dependent issue is spawnable.

## Convention

Typically for straightforward and well-defined tasks, we prefer setting the `auto` label such that said tasks are picked up and spawned by the jig daemon.
