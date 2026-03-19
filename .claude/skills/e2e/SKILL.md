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

Read `docs/DEBUG.md` for dev environment commands and debugging.

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
4. **FUSE detection**: `./bin/dev fuse-check` — reports whether FUSE tests will run
5. Verify fixtures on full node: `./bin/dev api full list` and `./bin/dev api full ls <id> /docs`
6. **Wait 60 seconds for sync**: `sleep 60`
7. Check cross-node sync on app: `./bin/dev api app list`
8. Check S3 gateway: `curl -s http://localhost:9093/gw/<bucket_id>/docs/readme.md?download=true`
9. Verify blobs in MinIO: `docker exec jax-minio mc ls local/jax-blobs/data/ | head -5`
10. Check for **real** errors: `./bin/dev logs grep ERROR` - ignore "No addressing information" (transient)

## FUSE Filesystem Tests

FUSE tests run automatically as part of the fixture system (mount → mount_verify → unmount) when FUSE is available. Use `./bin/dev fuse-check` to determine availability before interpreting fixture results.

**FUSE availability depends on two things:**
1. Platform support: `/dev/fuse` on Linux, `/Library/Filesystems/macfuse.fs` on macOS
2. Daemon built with fuse feature: check `_status/version` endpoint for `build_features`

**Reporting rules:**
- If FUSE is **not available**, report "FUSE tests skipped (not available on this machine)" — this is **NOT a failure**
- If FUSE **is available** but tests fail, this **IS a failure** and must be reported
- The test plan in the PR must explicitly state whether FUSE tests ran or were skipped

The `mount_verify` fixture tests these filesystem operations:
- Directory listing (ls)
- File read (head)
- File write (echo > file)
- File rename/mv (create → rename → verify)
- File overwrite (echo > existing_file)
- File delete (rm)

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

### FUSE Filesystem (if available)
- FUSE detected: [yes/no/skipped — not available on this machine]
- Mount created: [yes/no/skipped]
- Directory listing: [pass/fail/skipped]
- File read: [pass/fail/skipped]
- File write: [pass/fail/skipped]
- File rename (create → mv): [pass/fail/skipped]
- File overwrite: [pass/fail/skipped]
- File delete: [pass/fail/skipped]
- Unmount clean: [yes/no/skipped]

### Errors
[List REAL errors only - NOT "No addressing information available" which is transient]

### Summary
[PASS/FAIL] - [description]
```
