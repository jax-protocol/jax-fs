# Native File Dialog for Upload/Export

**Status:** Planned
**Follow-up from:** Gateway-Tauri-Split ticket 4 (Tauri desktop app)

## Objective

Add native file open/save dialogs for upload and export flows in the Tauri desktop app using `tauri-plugin-dialog`.

## Background

The desktop app currently handles upload and export through the browser-style UI. Native dialogs would provide a better UX: OS-native file picker for uploads, save dialog for exports.

## Implementation Steps

### 1. Add tauri-plugin-dialog

**File:** `crates/desktop/src-tauri/Cargo.toml`
- Add `tauri-plugin-dialog` dependency

**File:** `crates/desktop/src-tauri/src/lib.rs`
- Register the dialog plugin

### 2. Upload flow

- Add IPC command that opens a native file picker (multi-select)
- On selection, read file contents and call `add_files` API
- Wire into Explorer page upload button

### 3. Export flow

- Add IPC command that opens a native save dialog
- On confirm, call `export` API with chosen path
- Wire into bucket export action

## Acceptance Criteria

- [ ] Upload button opens native file picker
- [ ] Selected files are uploaded to current directory
- [ ] Export opens native save dialog
- [ ] Export writes to chosen path
- [ ] Works on macOS and Linux
