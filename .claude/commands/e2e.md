---
description: Run end-to-end dev environment tests
allowed-tools:
  - Bash(./bin/dev)
  - Bash(./bin/dev *)
  - Bash(tmux capture-pane *)
  - Bash(tmux has-session *)
  - Bash(sleep *)
  - Read
  - Grep
  - Glob
---

Run end-to-end tests of the dev environment to verify fixtures and cross-node sync.

## Overview

This skill enables autonomous e2e testing of the jax-bucket dev environment:
1. Manage dev environment lifecycle (kill, clean, run)
2. Wait for nodes to become healthy
3. Apply and verify fixtures
4. Debug issues using logs and API calls
5. Connect to tmux sessions to introspect build errors

## Dev Environment Commands

All dev commands go through `./bin/dev`:

```bash
./bin/dev              # Start environment (default: run)
./bin/dev run          # Start all nodes in tmux
./bin/dev kill         # Kill tmux session
./bin/dev clean        # Remove all dev data
./bin/dev status       # Check node status

./bin/dev api <node> <cmd>     # API calls (full, app, gw)
./bin/dev api full health      # Health check
./bin/dev api full list        # List buckets
./bin/dev api full ls <id> /   # List files
./bin/dev api full cat <id> /path  # Read file

./bin/dev logs tail            # Tail all logs
./bin/dev logs tail full       # Tail specific node
./bin/dev logs grep ERROR      # Search logs

./bin/dev fixtures list        # List fixtures
./bin/dev fixtures apply       # Apply fixtures
```

## Standard E2E Test Flow

### 1. Clean Start
```bash
./bin/dev kill
./bin/dev clean
```

### 2. Start Environment in Background
```bash
./bin/dev run --background
```
The dev script starts a tmux session `jax-dev` with 3 panes (one per node).
The `--background` flag prevents attaching to tmux, allowing autonomous testing.

### 3. Wait for Nodes
Poll health endpoints until all respond:
```bash
./bin/dev api full health
./bin/dev api app health
./bin/dev api gw health
```
All should return `{"status": "ok"}`.

### 4. Verify Fixtures Applied
Fixtures auto-apply on startup. Check results:
```bash
./bin/dev api full list        # Should show "test" bucket
./bin/dev api full ls <id> /   # Should show files
```

### 5. Verify Cross-Node Sync
```bash
./bin/dev api app list         # App node should see shared bucket
./bin/dev api app ls <id> /    # Should see same files
```

### 6. Check for Errors
```bash
./bin/dev logs grep ERROR
./bin/dev logs grep -i "unexpected"
```

## Expected Fixture State

Based on `bin/dev_/fixtures.toml`:

| Step | Node | Action | Result |
|------|------|--------|--------|
| 1 | full | Create bucket "test" | bucket_id created |
| 2 | full | Upload /hello.txt | File at root |
| 3 | full | Create /docs dir | Directory exists |
| 4 | full | Upload /docs/readme.md | Nested file |
| 5 | full | Share with app (owner) | App can read/write |
| 6 | full | Share with gw (mirror) | GW can sync |
| 7 | full | Publish bucket | Mirrors can decrypt |
| 8 | app | Move /hello.txt to /docs/ | File relocated |

**Final state on all nodes:**
- Bucket "test" exists
- `/docs/` directory
- `/docs/hello.txt` (moved)
- `/docs/readme.md`

## Debugging

### View Build Errors (tmux)
```bash
tmux capture-pane -t jax-dev:0 -p -S -100  # Node 0 (full)
tmux capture-pane -t jax-dev:1 -p -S -100  # Node 1 (app)
tmux capture-pane -t jax-dev:2 -p -S -100  # Node 2 (gw)
```

### View Logs
Log files are at `./data/node{0,1,2}/logs/jax.log.*`
```bash
./bin/dev logs tail full    # Live tail
./bin/dev logs grep ERROR   # Search for errors
cat ./data/node0/logs/jax.log.* | grep -i mount  # Search specific
```

### Common Issues

| Error | Cause | Fix |
|-------|-------|-----|
| "Mount error: head not found" | Bucket not synced | Check share/publish worked |
| "Unexpected error" | MountError swallowed | Check logs for actual error |
| "Gateway nodes do not expose API" | Correct behavior | Use full/app for bucket ops |
| Nodes not starting | Port conflict | `./bin/dev kill && ./bin/dev clean` |
| Fixture failed | API error | Check logs, verify node healthy |

## Running Background Dev for Ticket Work

When working on tickets that need a running dev environment:

1. Start in background: `./bin/dev run --background`
2. Wait for health: poll `./bin/dev api full health`
3. Do your work
4. Test changes: `./bin/dev fixtures apply` or manual API calls
5. Check logs: `./bin/dev logs grep ERROR`
6. When done: `./bin/dev kill`

## Report Format

After running e2e tests, report:

```
## E2E Test Results

### Node Health
- full: [OK/FAIL]
- app: [OK/FAIL]
- gw: [OK/FAIL]

### Fixtures
- Bucket created: [yes/no]
- Files uploaded: [yes/no]
- Share (owner): [yes/no]
- Share (mirror): [yes/no]
- Publish: [yes/no]
- Move operation: [yes/no]

### Cross-Node Sync
- App sees bucket: [yes/no]
- App can read files: [yes/no]

### Errors
[List any errors from logs]

### Summary
[PASS/FAIL] - [description]
```
