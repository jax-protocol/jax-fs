# Job queue saturation from stale peer pings blocks syncs

**Status:** Open
**Priority:** High
**Category:** Bugs
**Auto:** true

## Problem

The daemon's single-threaded job worker processes all jobs (pings, syncs, downloads) serially. When peers are stale/offline, each PingPeer job blocks for ~30s on iroh's connect timeout before failing. The periodic ping scheduler fires every 60s and dispatches pings for every (bucket x peer) combination simultaneously, flooding the bounded queue (capacity 1000).

### Symptoms

- `WARN: job queue is full - worker may be overloaded` spam
- `ERROR: Failed to connect to peer ... timed out` every 30s
- Sync jobs starved behind dozens of timing-out pings
- Very slow syncs between nodes

### Root causes

1. **No ping timeout** — `Ping::send()` in `ping_peer.rs:59` relies on iroh's ~30s connect timeout
2. **Serial execution** — `run_worker()` in `sync_provider.rs:127` processes one job at a time; one slow ping blocks everything
3. **Aggressive ping schedule** — 60s interval dispatches O(buckets x peers) pings per tick
4. **No batch dedup** — new periodic batch fires even if previous batch hasn't drained

## Implementation Steps

### 1. Add 5s timeout to ping execution

**File**: `crates/common/src/peer/sync/ping_peer.rs`

Wrap the `Ping::send()` call (line 59) in `tokio::time::timeout(Duration::from_secs(5), ...)`. Return `Ok(())` on timeout — peer unavailability is expected, not an error. Apply timeout at the ping job level (not in `bidirectional.rs`) so sync/download operations keep their full timeout.

### 2. Spawn ping jobs concurrently

**File**: `crates/daemon/src/sync_provider.rs`

In `run_worker()`, match on job type in the select loop. Spawn `PingPeer` jobs as tokio tasks capped by a semaphore (~10 concurrent). Execute `SyncBucket`/`DownloadPins` inline (serial). This way pings never block the worker from processing sync jobs.

Pattern already validated by `ping_and_collect()` in `peer_inner.rs:168`.

### 3. Increase periodic ping interval to 5 minutes

**File**: `crates/daemon/src/sync_provider.rs` (line 124)

Change `Duration::from_secs(60)` to `Duration::from_secs(300)`. Fix stale comment on line 123. Event-driven pings (`save_mount`) handle the normal case; periodic pings are a fallback.

### 4. Skip periodic batch if previous still running

**File**: `crates/daemon/src/sync_provider.rs`

Add an `Arc<AtomicBool>` flag. Set before spawning periodic ping task, clear when done. Skip scheduling if flag is set, preventing cascading batches.

## Files to Modify

- `crates/common/src/peer/sync/ping_peer.rs` — 5s timeout wrapper around `Ping::send()`
- `crates/daemon/src/sync_provider.rs` — concurrent ping spawning, interval increase, skip guard

## Future Work

Local peer reliability scoring — track consecutive ping failures per peer and skip pinging known-dead peers. Similar to the EigenTrust approach from the earlier protocol design (see https://jax.ac/docs/consensus/peer_trust/).

## Verification

1. `cargo build && cargo test && cargo clippy && cargo fmt --check`
2. Run daemon with multiple buckets shared with stale peers
3. Confirm sync jobs complete promptly after `save_mount` without queue-full warnings
4. Confirm periodic pings fire at 5-minute intervals and skip if previous batch is active
