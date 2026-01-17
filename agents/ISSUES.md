# Issues and Tickets

Guide for organizing work in the `issues/` directory.

---

## Directory Structure

```
issues/
├── standalone-task.md              # Standalone issue (single task)
├── gateway-local-split/            # Epic (directory)
│   ├── index.md                    # Epic overview
│   ├── 0-service-crate.md          # First ticket
│   ├── 1-http-handlers.md          # Second ticket
│   └── 2-cli-integration.md        # Third ticket
└── fuse-mount-system/
    ├── index.md
    ├── 0-basic-mount.md
    └── 1-write-support.md
```

---

## Issue Types

### Standalone Issues

Simple tasks that don't need multiple tickets.

- **Location**: `issues/descriptive-name.md`
- **Use when**: Task is small enough for one session
- **Lifecycle**: Delete when done, or mark complete for auditing

### Epics

Large features broken into multiple tickets.

- **Location**: `issues/epic-name/index.md`
- **Contains**: Background, architecture decisions, ticket list
- **Tickets**: Numbered files in same directory (`0-`, `1-`, `2-`, ...)

### Tickets

Individual tasks within an epic.

- **Naming**: `N-descriptive-name.md` (0-indexed)
- **Purpose**: Everything needed to implement one task
- **Order**: Numbers suggest execution order

---

## Ticket Format

```markdown
# [Ticket Title]

**Status:** Planned | In Progress | Complete

## Objective

One-sentence description of what this accomplishes.

## Implementation Steps

1. Step-by-step guide
2. With specific file paths
3. And code snippets where helpful

## Files to Modify/Create

- `path/to/file.rs` - Description of changes

## Acceptance Criteria

- [ ] Checkbox criteria
- [ ] That can be verified

## Verification

How to test that this is working.
```

---

## Epic Index Format

```markdown
# [Epic Title]

## Background

Why this work is needed.

## Tickets

| # | Ticket | Status |
|---|--------|--------|
| 0 | [Service crate](./0-service-crate.md) | Complete |
| 1 | [HTTP handlers](./1-http-handlers.md) | In Progress |
| 2 | [CLI integration](./2-cli-integration.md) | Planned |

## Architecture Decisions

Key technical decisions made for this epic.
```

---

## Status Values

| Status | Meaning |
|--------|---------|
| `Planned` | Ready to be worked on |
| `In Progress` | Currently being implemented |
| `Complete` | Done and verified |
| `Blocked` | Waiting on external dependency |

---

## Lifecycle

### Completing work

When a ticket or issue is done:

1. Mark status as `Complete`
2. Optionally delete the file (keeps `issues/` clean)
3. Or keep it for auditing past work

### Creating new work

1. **Simple task**: Create `issues/descriptive-name.md`
2. **Large feature**: Create `issues/epic-name/` directory with `index.md`
3. **Add tickets**: Create `0-first-task.md`, `1-second-task.md`, etc.

---

## Best Practices

- Keep tickets small enough to complete in one session
- Use 0-indexed numbering for execution order
- Reference specific file paths in implementation steps
- Update status immediately when starting/finishing
- Run `cargo test && cargo clippy` before marking complete
