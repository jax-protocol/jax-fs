# Issues and Tickets

Guide for AI agents and contributors on how issues are organized in this repository.

---

## Directory Structure

```
issues/
├── public-buckets.md               # Epic: Public buckets & mirror peers
├── public-01-api-endpoints.md      # Ticket: API endpoints for publish/mirror
├── public-02-sync-protocol.md      # Ticket: Sync protocol for mirrors
├── public-03-gateway-access.md     # Ticket: Gateway access control
├── binary-split.md                 # Epic: Split gateway and local binaries
├── split-01-cleanup-app.md         # Ticket: Remove old app crate
└── ...
```

---

## Issue Types

### Epics

Large features or initiatives broken into multiple tickets.

- **File naming**: `feature-name.md` (descriptive, no number prefix)
- **Purpose**: High-level overview, context, and architecture decisions
- **Contains**: Background, phases, key technical decisions, verification checklists
- **Links to**: Child tickets for each discrete task

### Tickets

Focused, actionable tasks that can be completed in a single session.

- **File naming**: `feature-NN-short-description.md` (e.g., `public-02-sync-protocol.md`)
- **Number prefix**: Suggests execution order within a feature
- **Purpose**: Everything needed to implement one specific task
- **Links to**: Parent epic for full context

---

## Ticket Format

```markdown
# [Ticket Title]

**Status:** Planned | In Progress | Complete
**Epic:** [epic-name.md](./epic-name.md)
**Dependencies:** ticket-01 (if any)

## Objective

One-sentence description of what this ticket accomplishes.

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

## Picking Up Work

### Finding available tickets

1. Look in `issues/` for tickets with `Status: Planned`
2. Check the parent epic to understand the broader context
3. Verify dependencies are complete before starting

### Working on a ticket

1. Update the ticket status to `In Progress`
2. Read the parent epic for context
3. Follow the implementation steps
4. Check off acceptance criteria as you go
5. Run `cargo build --workspace && cargo test --workspace` before marking complete
6. Update status to `Complete`

### Creating new tickets

1. If working on an existing epic, follow its naming pattern (e.g., `public-03-...`)
2. For new features, consider creating an epic first if the scope is large
3. Use the ticket format template above

---

## Status Values

| Status | Meaning |
|--------|---------|
| `Planned` | Ready to be worked on |
| `In Progress` | Currently being implemented |
| `Complete` | Done and verified |
| `Blocked` | Waiting on external dependency |

---

## Dependencies

Tickets can have dependencies in two ways:

1. **Implicit (number order)**: `public-01` should be done before `public-02`
2. **Explicit**: Listed in the Dependencies field when non-linear

Example:
```markdown
**Dependencies:** public-01-api-endpoints, split-01-cleanup-app
```

---

## Best Practices

- Keep tickets small enough to complete in one session
- Reference specific file paths in implementation steps
- Include code snippets for complex changes
- Always link back to the parent epic
- Update status immediately when starting/finishing work
