---
description: Run end-to-end dev environment tests
allowed-tools:
  - Bash(./bin/dev)
  - Bash(./bin/dev *)
  - Bash(curl *)
  - Bash(docker exec jax-minio *)
  - Bash(tmux capture-pane *)
  - Bash(tmux has-session *)
  - Bash(sleep *)
  - Bash(echo *)
  - Read
  - Grep
  - Glob
---

Run end-to-end tests of the dev environment to verify fixtures and cross-node sync.

Read `agents/DEBUG.md` for dev environment commands and debugging.

**Expected end state is documented in `bin/dev_/fixtures.toml`** - see the "EXPECTED END STATE" comment at the end of that file for what to verify.

## IMPORTANT: Sync Timing

**Be patient with sync.** P2P discovery and sync takes time in local dev:
- Wait **at least 60 seconds** after fixtures before checking cross-node sync
- "No addressing information available" errors are **transient** - they resolve as peers discover each other
- If app node shows empty bucket list, wait longer (up to 2 minutes)
- These are NOT errors, just discovery in progress

## E2E Test Flow

1. `./bin/dev kill --force && ./bin/dev clean` - Clean start
2. `./bin/dev run --background` - Start nodes
3. Wait for health: `./bin/dev api full health`
4. Verify fixtures on full node: `./bin/dev api full list` and `./bin/dev api full ls <id> /docs`
5. **Wait 60 seconds for sync**: `sleep 60`
6. Check cross-node sync on app: `./bin/dev api app list`
7. Check S3 gateway: `curl -s http://localhost:9093/gw/<bucket_id>/docs/readme.md?download=true`
8. Verify blobs in MinIO: `docker exec jax-minio mc ls local/jax-blobs/data/ | head -5`
9. Check for **real** errors: `./bin/dev logs grep ERROR` - ignore "No addressing information" (transient)

## Report Format

```
## E2E Test Results

### Node Health
- full: [OK/FAIL]
- app: [OK/FAIL]
- gw: [OK/FAIL]

### Fixtures (on full node)
- Bucket created: [yes/no]
- Files uploaded: [yes/no]
- Move operation: [yes/no]

### Cross-Node Sync (after 60s wait)
- App sees bucket: [yes/no]
- App can read files: [yes/no]
- Gateway (S3) sees bucket: [yes/no]
- Gateway (S3) can read files: [yes/no]

### S3 Storage
- Blobs in MinIO: [yes/no]

### Errors
[List REAL errors only - NOT "No addressing information available" which is transient]

### Summary
[PASS/FAIL] - [description]
```
