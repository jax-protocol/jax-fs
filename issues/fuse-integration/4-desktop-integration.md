# Desktop App Mount Integration

**Status:** Planned
**Track:** Local
**Depends on:** Ticket 1 (FUSE filesystem — REST API must be stable)

## Objective

Add FUSE mount management to the Tauri desktop app: IPC commands proxying to the daemon REST API, and a SolidJS Mounts page for viewing and controlling mounts.

## Implementation Steps

### 1. IPC commands

**Create:** `crates/desktop/src-tauri/src/commands/mount.rs`

| IPC Command | Maps to | Description |
|-------------|---------|-------------|
| `list_mounts` | `GET /api/v0/mounts/` | List all mounts |
| `create_mount` | `POST /api/v0/mounts/` | Create mount config |
| `start_mount` | `POST /api/v0/mounts/:id/start` | Start FUSE process |
| `stop_mount` | `POST /api/v0/mounts/:id/stop` | Stop FUSE process |
| `delete_mount` | `DELETE /api/v0/mounts/:id` | Delete mount config |

**Modify:** `crates/desktop/src-tauri/src/lib.rs` — Register mount commands

### 2. TypeScript API types

**Modify:** `crates/desktop/src/lib/api.ts`

Add mount types and API functions:
- `FuseMount` type (mount_id, bucket_id, mount_point, status, auto_mount, etc.)
- `listMounts()`, `createMount()`, `startMount()`, `stopMount()`, `deleteMount()`

### 3. Mounts page

**Create:** `crates/desktop/src/pages/Mounts.tsx`

- List of configured mounts showing: bucket name, mount point, status (stopped/running/error)
- Status indicators: green for running, gray for stopped, red for error
- Start/stop toggle button per mount
- "Add Mount" button opening a dialog
- Delete mount with confirmation

### 4. Add mount dialog

- Bucket selector (dropdown of available buckets)
- Mount point path input (with native directory picker via `tauri-plugin-dialog`)
- Auto-mount checkbox
- Read-only checkbox

### 5. Wire into navigation

**Modify:** `crates/desktop/src/` (App layout / sidebar)
- Add "Mounts" navigation item

## Files Summary

| Action | Path |
|--------|------|
| Create | `crates/desktop/src-tauri/src/commands/mount.rs` |
| Create | `crates/desktop/src/pages/Mounts.tsx` |
| Modify | `crates/desktop/src-tauri/src/lib.rs` (register commands) |
| Modify | `crates/desktop/src/lib/api.ts` (mount types + functions) |
| Modify | App layout / router (add Mounts page) |

## Acceptance Criteria

- [ ] IPC commands for mount CRUD + start/stop work
- [ ] Mounts page lists all configured mounts
- [ ] Status indicators show running/stopped/error
- [ ] Start/stop toggle works
- [ ] Add mount dialog creates new mount config
- [ ] Path picker uses native dialog
- [ ] Delete mount with confirmation works
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings
