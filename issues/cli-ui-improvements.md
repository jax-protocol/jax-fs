# CLI: improve output formatting and add shared UI module

**Status:** Planned
**Priority:** Medium
**Category:** Features
**Auto:** true

## Objective

Create a shared CLI UI module with consistent formatting patterns, better table styling, and status indicators across all commands.

## Background

Currently each CLI command implements `fmt::Display` independently with ad-hoc styling via `owo_colors`. There is no shared UI module, no consistent status symbols, no column styling on tables, and no truncation helpers. The `jig` project (https://github.com/amiller68/jig) demonstrates a clean approach with a centralized `ui.rs` module that this project should adopt.

### Current state

- Commands: `bucket list`, `bucket ls`, `shares ls`, `mount list`, `health`, `create`, `add`, `update`, etc.
- Tables use bare `comfy-table` with no column styling, alignment, or truncation
- Status messages use inline `owo_colors` calls with no consistent symbol set
- No plain output mode for scripting (`--plain` flag)
- `indicatif` is available but only used for the `update` spinner

### What jig does well

- Centralized `ui.rs` with color mappings, table builders, and truncation helpers
- Consistent status symbols: `✓` (green/success), `→` (cyan/progress), `✗` (red/failure)
- Column-specific styling: colored values, right-alignment for numbers, truncation for long strings
- `--plain` flag for scriptable output (bare values, newline-separated)
- Dual streams: data to stdout, status messages to stderr

## Implementation Steps

### 1. Create a shared UI module

Add `crates/daemon/src/cli/ui.rs` with:
- Status symbol constants: `✓`, `→`, `✗`, `!`
- Color mapping helpers for common patterns (success, warning, error, dimmed labels)
- String truncation helper (e.g., truncate hash strings to 16 chars with ellipsis)
- Table builder helper that applies consistent column styling

### 2. Improve table output

For `bucket list`:
- Style NAME in bold/white, ID dimmed or truncated, LINK truncated

For `bucket ls`:
- Color TYPE column (directories in blue, files in white)
- Truncate HASH column
- Right-align SIZE if added

For `mount list`:
- Color STATUS column (green for running, red for stopped)
- Highlight AUTO and RO flags

For `shares ls`:
- Color ROLE column (owner in yellow, writer in cyan, reader in white)

### 3. Add consistent status messages

Standardize success/progress/error formatting across commands:
```
✓ Created bucket "my-bucket"
→ Uploading file.txt...
✗ Failed to connect to daemon
```

Replace current inline styling like `"Created".green().bold()` with shared helpers.

### 4. Add `--plain` flag

Add a global `--plain` flag that:
- Outputs bare values (tab-separated or newline-separated) instead of tables
- Omits colors and decorations
- Enables piping output to other tools

### 5. Improve error display

- Use `✗` symbol prefix for errors
- Show error chains with indented "caused by:" lines
- Dim stack-trace-like details

## Files to Modify/Create

- `crates/daemon/src/cli/ui.rs` - New shared UI module
- `crates/daemon/src/cli/mod.rs` - Add `ui` module
- `crates/daemon/src/cli/args.rs` - Add `--plain` global flag
- `crates/daemon/src/cli/ops/bucket/list.rs` - Styled table output
- `crates/daemon/src/cli/ops/bucket/ls.rs` - Styled table output
- `crates/daemon/src/cli/ops/bucket/create.rs` - Use status symbols
- `crates/daemon/src/cli/ops/bucket/add.rs` - Use status symbols
- `crates/daemon/src/cli/ops/bucket/shares/ls.rs` - Styled table output
- `crates/daemon/src/cli/ops/mount/list.rs` - Styled table, colored status
- `crates/daemon/src/cli/ops/health.rs` - Use status symbols
- `crates/daemon/src/main.rs` - Pass plain flag through OpContext

## Acceptance Criteria

- [ ] Shared `ui.rs` module exists with symbol constants, color helpers, and truncation
- [ ] All table commands use styled columns (color, alignment, truncation)
- [ ] All success/error messages use consistent status symbols
- [ ] `--plain` flag outputs bare values suitable for scripting
- [ ] `mount list` shows colored status (green/red)
- [ ] `shares ls` shows colored roles
- [ ] No regressions: `cargo test` and `cargo clippy` pass

## Verification

1. Run `jax bucket list` and confirm styled table output
2. Run `jax bucket list --plain` and confirm bare output
3. Run `jax mount list` with running/stopped mounts and confirm colored status
4. Run `jax bucket create test` and confirm `✓` symbol in output
5. Pipe `jax bucket list --plain` to `wc -l` and confirm it works
