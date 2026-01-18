# Desktop Integration

**Status:** Planned
**Track:** Local

## Objective

Add desktop integration to `jax daemon`: system tray, startup on boot, native notifications.

## Implementation Steps

1. Add desktop feature flag to app crate
2. Integrate `tray-icon` crate for system tray
3. Integrate `auto-launch` crate for startup on boot
4. Add tray menu (open UI, show status, quit)
5. Add sync status indicator
6. Add native notifications for sync events

## Dependencies

- `tray-icon` - Cross-platform system tray
- `auto-launch` - Startup on boot
- `notify-rust` - Native notifications (Linux/macOS)

## Files to Modify

| File | Changes |
|------|---------|
| `crates/app/Cargo.toml` | Add dependencies with feature flag |
| `crates/app/src/daemon/mod.rs` | Initialize tray, autostart |
| `crates/app/src/daemon/tray.rs` | New file, tray menu handling |

## Acceptance Criteria

- [ ] `jax daemon --desktop` shows tray icon
- [ ] Tray menu has: Open UI, Status, Quit
- [ ] App can be configured to start on boot
- [ ] Sync events show notifications
- [ ] Works on macOS and Linux
- [ ] Feature-gated (can compile without desktop deps)

## Verification

```bash
# Build with desktop feature
cargo build --features desktop

# Run with tray
cargo run -- daemon --desktop

# Verify tray icon appears
# Verify menu works
# Configure autostart, reboot, verify starts
```
