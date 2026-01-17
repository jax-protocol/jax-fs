# Tauri Desktop App

**Status:** In Progress (exploration/prototype)
**Epic:** None (standalone feature)
**Dependencies:** None

## Reference Implementation

Branch: [`amiller68/tauri-app-explore`](https://github.com/jax-protocol/jax-buckets/tree/amiller68/tauri-app-explore)

Draft Tauri app with SolidJS frontend. Runs full P2P peer with IPC commands.

## Objective

Create a native desktop application for local bucket management using Tauri, separate from the CLI daemon which focuses on gateway/server use cases.

## Architecture Decision: Two Separate Apps

### 1. CLI Daemon (`jax daemon`)
- **Purpose**: Gateway/server use case
- **Features**: Full HTTP servers (HTML UI + REST API), P2P peer, sync
- **Use case**: Hosting buckets, serving content over HTTP, headless/server deployments

### 2. Tauri Desktop App
- **Purpose**: Local desktop management
- **Features**: Native GUI, system tray, full P2P peer, sync
- **Use case**: Personal bucket management, desktop users
- **No HTTP servers** - uses Tauri IPC for all operations

**Rationale**: Gateway serving (public HTTP access) is fundamentally different from local desktop management. Keeping them separate allows each to be optimized for its use case. Both run full P2P peers that can sync with each other.

## Technical Stack

### Backend (Rust)
- **Tauri 2.0** - Desktop framework with native async support
- **Shared `AppState`/`AppConfig`** - Same config loading as CLI
- **Full P2P peer** - iroh networking for sync
- **`--config-path` argument** - Supports multi-node testing (matches CLI interface)

### Frontend
- **SolidJS** - React-like DX, smaller bundle, better performance
- **TypeScript** - Type safety
- **Tailwind CSS** - Styling
- **Vite** - Build tooling

### Features
- **System tray icon** - App runs in background
- **Autostart** - `tauri-plugin-autostart` for launch on login
- **Native file dialogs** - For file uploads
- **Full CRUD operations** via IPC commands

## Project Structure

```
crates/tauri/
├── src-tauri/           # Tauri Rust backend
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/
│   └── src/
│       ├── lib.rs       # Main Tauri setup, tray, lifecycle
│       └── commands.rs  # IPC command handlers
├── src/                 # SolidJS frontend
│   ├── App.tsx          # Root with routing
│   ├── pages/
│   │   ├── BucketsList.tsx
│   │   ├── BucketExplorer.tsx
│   │   └── FileViewer.tsx
│   └── index.tsx
├── package.json
├── vite.config.ts
└── index.html
```

## Shared Code Changes

To support both CLI and Tauri using the same config:

| File | Changes |
|------|---------|
| `crates/app/src/daemon/app_state.rs` | Shared config loading |
| `crates/app/src/lib.rs` | Library exports for Tauri |
| `crates/app/src/daemon/types.rs` | Shared `PathHashMap` type |

Both apps can:
- Use the same `~/.jax/` config directory
- Share database and blob stores
- Run full P2P peers that sync with each other

## IPC Commands Implemented

| Command | Description |
|---------|-------------|
| `list_buckets` | Get all buckets |
| `create_bucket` | Create new bucket |
| `get_bucket` | Get bucket details |
| `list_files` | List files in bucket |
| `get_file` | Get file content |
| `add_file` | Upload file |
| `delete_file` | Delete file |
| `rename_file` | Rename file |
| `move_file` | Move file |
| `create_directory` | Create directory |

## Frontend Pages

1. **Buckets List** (`/`) - Grid of buckets with create modal
2. **Bucket Explorer** (`/buckets/:id`) - File browser with breadcrumbs, upload, actions
3. **File Viewer** (`/buckets/:id/view`) - Text, code, images, binary preview

## Multi-Node Testing

The `--config-path` argument enables running multiple nodes locally:

```bash
# Terminal 1 - Node A
cargo tauri dev -- -- --config-path ~/.jax-a

# Terminal 2 - Node B
cargo tauri dev -- -- --config-path ~/.jax-b
```

Matches the CLI's interface and supports the `bin/dev.sh` testing workflow.

## How to Run

```bash
cd crates/tauri
npm install
cargo tauri dev
```

## Key Files to Reference

| File | Description |
|------|-------------|
| `crates/tauri/src-tauri/src/lib.rs` | Main Tauri setup, ServiceState initialization |
| `crates/tauri/src-tauri/src/commands.rs` | IPC command implementations |
| `crates/app/src/daemon/app_state.rs` | Shared config loading logic |
| `crates/app/src/lib.rs` | Library exports |

## Acceptance Criteria

### Completed
- [x] Tauri 2.0 project structure
- [x] Shared config with CLI daemon
- [x] Full P2P peer integration
- [x] System tray icon
- [x] IPC commands for all CRUD operations
- [x] SolidJS frontend with routing
- [x] Buckets list page
- [x] File explorer page
- [x] File viewer page
- [x] Multi-node testing support

### Remaining
- [ ] Frontend polish and UX improvements
- [ ] Improved error handling
- [ ] File upload progress indicator
- [ ] Sync status indicator in tray
- [ ] CodeMirror editor integration for editing
- [ ] macOS DMG packaging
- [ ] Linux AppImage packaging
- [ ] Windows installer packaging

## Verification

```bash
# Development mode
cd crates/tauri
npm install
cargo tauri dev

# Build for distribution
cargo tauri build

# Multi-node testing
cargo tauri dev -- -- --config-path ~/.jax-test
```
