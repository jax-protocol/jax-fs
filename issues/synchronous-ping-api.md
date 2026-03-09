# Synchronous ping API and peer status UI

**Status:** Open
**Priority:** High
**Category:** Feature

## Problem

The current ping API (`POST /api/v0/bucket/ping`) is fire-and-forget: it dispatches a `PingPeerJob` to a background queue and immediately returns `{"success": true, "message": "Ping job dispatched"}`. There is no way to see what happened — whether the peer was reached, what its sync status is, or if the connection failed entirely.

The desktop SharePanel has a per-peer "Ping" button that calls this endpoint. It shows "Ping job dispatched" every time, which is useless for debugging sync issues. The embedded daemon has no file logging (`log_dir: None`), so there's no way to observe what's happening.

## Fix

### 1. Replace the fire-and-forget ping API with a synchronous one

`Peer::ping_and_collect()` already exists (`crates/common/src/peer/peer_inner.rs:168`) — it pings all peers concurrently and returns a map of `public_key -> PingReplyStatus`. Wire this up as the ping API handler instead of dispatching to the background queue.

**File**: `crates/daemon/src/http_server/api/v0/bucket/ping.rs`

- Change `PingRequest` to only take `bucket_id` (no `peer_public_key` — ping all peers)
- Change `PingResponse` to return a list of `PeerStatus { public_key, status, height }` where status is `in_sync | not_found | behind | ahead`
- Handler calls `peer.ping_and_collect(bucket_id, Some(Duration::from_secs(15)))` and maps the result
- Use `common::peer::PingReplyStatus` (re-exported, not the private `protocol` path)

### 2. Update Tauri command

**File**: `crates/desktop/src-tauri/src/commands/bucket.rs`

- `ping_peer` command drops the `peer_public_key` param (pings all peers now)
- Returns serialized JSON of the peer statuses instead of a message string

### 3. Update frontend API

**File**: `crates/desktop/src/lib/api.ts`

- Replace `pingPeer(bucketId, peerPublicKey)` with `pingPeers(bucketId)` returning `PeerStatus[]`
- Add `PeerStatus` interface: `{ public_key: string, status: string, height: number | null }`

### 4. Update SharePanel UI

**File**: `crates/desktop/src/components/SharePanel.tsx`

- Replace per-peer "Ping" buttons with a single "Ping All" button in the Peers section header
- After pinging, show a status badge next to each peer (synced/not found/behind/ahead) with color coding
- Remove the generic `pingResult` text display at the bottom

## Files to Modify

1. `crates/daemon/src/http_server/api/v0/bucket/ping.rs` — synchronous handler using `ping_and_collect`
2. `crates/desktop/src-tauri/src/commands/bucket.rs` — update Tauri command signature
3. `crates/desktop/src/lib/api.ts` — new `pingPeers` function and `PeerStatus` type
4. `crates/desktop/src/components/SharePanel.tsx` — "Ping All" button with per-peer status badges
