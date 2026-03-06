# Desktop Log Viewer

**Status:** Planned
**Priority:** Medium
**Category:** Features
**Auto:** true

## Objective

Add a log viewer page to the desktop app so users can stream daemon logs in real-time and browse historical log files without leaving the UI.

## Background

When the daemon runs embedded in the desktop app, `log_dir` is set to `None` (`crates/desktop/src-tauri/src/lib.rs:245`), so logs only go to stdout and are invisible to the user. Even in sidecar mode, users must manually find and tail log files. This makes debugging slow syncs and other issues difficult.

The daemon already uses `tracing` with a daily rolling file appender (`crates/daemon/src/process/mod.rs:54-110`). The frontend already has access to `jax_dir` via `ConfigInfo` (`crates/desktop/src/lib/api.ts:148-157`).

## Implementation Steps

### 1. Enable log files in embedded mode

- In `crates/desktop/src-tauri/src/lib.rs:245`, set `log_dir` to a subdirectory of `jax_dir` (e.g., `jax_dir.join("logs")`) instead of `None`
- This ensures the rolling file appender in `crates/daemon/src/process/mod.rs` writes log files to disk

### 2. Add Tauri commands for log access

- Create `crates/desktop/src-tauri/src/commands/logs.rs` with:
  - `list_log_files` - List available log files from the log directory, returning filename, size, and modified date
  - `read_log_file` - Read contents of a specific log file with optional line limit and offset for pagination
  - `tail_log_file` - Read the last N lines of the current log file for live-tail initial load
- Register commands in the Tauri builder

### 3. Add a log streaming channel

- Use Tauri's event system to push new log lines to the frontend in real-time
- Add a `tracing` layer that emits log events over the Tauri event channel when the frontend subscribes
- Add `subscribe_logs` / `unsubscribe_logs` Tauri commands to control the stream

### 4. Create the Logs page in the frontend

- Add `crates/desktop/src/pages/Logs.tsx` (SolidJS component) with:
  - **Live tail tab**: Auto-scrolling log output with pause/resume, severity-level color coding, and text filter
  - **Log files tab**: List of historical log files with click-to-view and in-file search
- Add navigation entry alongside existing pages (Settings, Explorer, History, etc.)
- Add route in the app router

### 5. Add log level filter controls

- Dropdown or toggles to filter by log level (ERROR, WARN, INFO, DEBUG, TRACE)
- Text search/filter input for narrowing log output
- Apply filters both to live tail and file browsing views

## Files to Modify/Create

- `crates/desktop/src-tauri/src/lib.rs` - Set `log_dir` for embedded mode
- `crates/desktop/src-tauri/src/commands/logs.rs` - New Tauri commands for log access
- `crates/desktop/src-tauri/src/commands/mod.rs` - Register log commands
- `crates/desktop/src/pages/Logs.tsx` - New log viewer page
- `crates/desktop/src/lib/api.ts` - Add API bindings for log commands
- App router and navigation - Add Logs page route and nav entry

## Acceptance Criteria

- [ ] Embedded mode writes log files to disk under `jax_dir/logs/`
- [ ] `list_log_files` command returns available log files with metadata
- [ ] `read_log_file` command returns paginated log file contents
- [ ] Live tail streams new log lines to the frontend in real-time
- [ ] Logs page renders with live tail and file browser tabs
- [ ] Log level filtering works for both live and historical views
- [ ] Text search filters log output
- [ ] Navigation includes the Logs page
- [ ] `cargo build` compiles without errors
- [ ] `cargo clippy` passes without warnings
- [ ] `cargo test` passes

## Verification

1. Launch the desktop app in embedded mode
2. Navigate to the Logs page
3. Confirm live tail shows log output updating in real-time
4. Trigger a sync operation and verify related log lines appear
5. Switch to the file browser tab and confirm historical log files are listed
6. Open a log file and verify contents render with search working
7. Toggle log level filters and confirm output updates correctly
