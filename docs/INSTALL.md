# Installation Guide

This guide covers installation and system requirements for JaxBucket.

## System Requirements

### Operating Systems

- **Linux**: Any modern distribution (Ubuntu 20.04+, Debian 11+, Fedora 35+, etc.)
- **macOS**: 10.15 (Catalina) or later
- **Windows**: Windows 10/11 with WSL2 recommended (native Windows support is experimental)

### Software Requirements

- **Rust**: Version 1.75 or later (2021 edition)
- **Cargo**: Comes with Rust installation
- **Git**: For cloning the repository

### System Libraries

JaxBucket requires the following system libraries:

#### Linux (Ubuntu/Debian)
```bash
sudo apt update
sudo apt install build-essential pkg-config libssl-dev libsqlite3-dev
```

#### Linux (Fedora/RHEL)
```bash
sudo dnf install gcc pkg-config openssl-devel sqlite-devel
```

#### Linux (Gentoo)
```bash
emerge -av dev-lang/rust dev-libs/openssl dev-db/sqlite sys-fs/fuse:3
```

**Kernel note:** Ensure your kernel has FUSE support enabled (`CONFIG_FUSE_FS=y` or `CONFIG_FUSE_FS=m`). If built as a module, load it with `modprobe fuse`. Your user must also be in the `fuse` group:
```bash
gpasswd -a YOUR_USERNAME fuse
```

#### macOS
```bash
# Install Homebrew if not already installed
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install dependencies (most come with Xcode Command Line Tools)
brew install openssl sqlite3
```

#### Windows (WSL2)
Follow the Linux (Ubuntu/Debian) instructions above within your WSL2 environment.

### Hardware Requirements

**Minimum:**
- CPU: 2 cores
- RAM: 2 GB
- Disk: 500 MB for binaries + storage for your encrypted data

**Recommended:**
- CPU: 4+ cores
- RAM: 4+ GB
- Disk: 10+ GB for comfortable operation
- Network: Stable internet connection for P2P sync

## Installation

### Desktop App (Recommended for most users)

Download pre-built binaries from GitHub releases:

1. Go to [GitHub Releases](https://github.com/jax-protocol/jax-fs/releases)
2. Find the latest `jax-desktop-v*` release
3. Download the installer for your platform:

| Platform | File | FUSE Mount Support | Install |
|----------|------|--------------------|---------|
| macOS (Apple Silicon) | `Jax_*_aarch64.dmg` | No | Open DMG, drag to Applications |
| macOS (Apple Silicon + FUSE) | `Jax_*_aarch64_fuse.dmg` | Yes | Open DMG, drag to Applications |
| macOS (Intel) | `Jax_*_x64.dmg` | No | Open DMG, drag to Applications |
| Linux (Debian/Ubuntu) | `jax-desktop_*_amd64.deb` | No | `sudo dpkg -i jax-desktop_*.deb` |
| Linux (portable) | `jax-desktop_*_amd64.AppImage` | No | `chmod +x *.AppImage && ./*.AppImage` |

The `_fuse` variant includes FUSE mount support, which lets you mount buckets as local filesystem directories. FUSE mount support is currently only available on macOS Apple Silicon and requires [macFUSE](https://osxfuse.github.io/) to be installed. All other builds work without any FUSE dependencies.

**macOS Gatekeeper:** The app is not yet Apple-signed, so macOS will block it on first launch with "'Jax' is damaged and can't be opened." To fix this, remove the quarantine attribute after installing:

```bash
xattr -cr /Applications/Jax.app
```

Alternatively, you can right-click the app and select "Open", or go to System Settings > Privacy & Security and click "Open Anyway".

#### Building Desktop App from Source

If you prefer to build from source:

```bash
# Clone the repository
git clone https://github.com/jax-protocol/jax-fs.git
cd jax-fs/crates/desktop

# Install frontend dependencies
pnpm install

# Build the app
pnpm tauri build
```

The built installer will be in `target/release/bundle/`:
- macOS: `dmg/*.dmg`
- Linux: `deb/*.deb` or `appimage/*.AppImage`

##### Installing a Local Build to Applications

On macOS, you can copy the built app bundle directly to your Applications folder:

```bash
# From the jax-fs repo root, after running pnpm tauri build
cp -r target/release/bundle/macos/Jax.app /Applications/

# Remove quarantine attribute (required for unsigned builds)
xattr -cr /Applications/Jax.app
```

On Linux, install the `.deb` directly:

```bash
sudo dpkg -i target/release/bundle/deb/jax-desktop_*.deb
```

Or run the AppImage without installing:

```bash
chmod +x target/release/bundle/appimage/jax-desktop_*.AppImage
./target/release/bundle/appimage/jax-desktop_*.AppImage
```

#### Building Without FUSE

The default desktop build includes FUSE support and requires FUSE libraries (macFUSE on macOS, libfuse3-dev on Linux). To build without FUSE:

```bash
cd jax-fs/crates/desktop
pnpm tauri build -- --no-default-features --features custom-protocol
```

Or for the daemon CLI only:
```bash
cargo build --release --no-default-features
```

**What's different without FUSE:** The `mount` command and FUSE filesystem features are unavailable. All other functionality (bucket creation, file operations, encryption, P2P sync) works normally.

**When to use a non-FUSE build:**
- Your system doesn't support FUSE (Windows, some Linux configurations)
- You don't need to mount buckets as local filesystem directories
- You want to avoid installing FUSE dependencies

**FUSE platform support:** FUSE mount support is currently only offered on macOS Apple Silicon. Pre-built FUSE binaries are not provided for other platforms.

**Gentoo desktop build dependencies:** The Tauri build requires WebKit, tray icon support, and SVG rendering:
```bash
emerge -av net-libs/webkit-gtk:4.1 dev-libs/libappindicator gnome-base/librsvg dev-util/patchelf
```
You will also need Node.js 20+ and pnpm. Install via your preferred method (e.g., `emerge -av net-libs/nodejs` or use [nvm](https://github.com/nvm-sh/nvm)), then `npm install -g pnpm`.

### CLI Installation

For headless servers or if you prefer the command line:

##### Option 1: Install Script (Recommended)

Install or update with a single command (no Rust toolchain required):

```bash
curl -fsSL https://raw.githubusercontent.com/jax-protocol/jax-fs/main/install.sh | sh
```

Install with FUSE mount support (macOS Apple Silicon only):

```bash
curl -fsSL https://raw.githubusercontent.com/jax-protocol/jax-fs/main/install.sh | sh -s -- --fuse
```

Install a specific version:

```bash
curl -fsSL https://raw.githubusercontent.com/jax-protocol/jax-fs/main/install.sh | sh -s -- --version 0.1.9
```

Re-running the script updates to the latest version. The binary is installed to `~/.local/bin` by default (set `JAX_INSTALL_DIR` to change). On interactive terminals, the script will prompt to install the FUSE variant on supported platforms.

##### Option 2: Install from Crates.io

For Rust developers who prefer cargo:

```bash
cargo install jax-daemon
```

This will download, compile, and install the `jax` binary to `~/.cargo/bin/`.

##### Option 3: Install from Git Repository

Install the latest development version:

```bash
cargo install --git https://github.com/jax-protocol/jax-fs jax-daemon
```

##### Option 4: Build from Source

Clone and build manually for development or customization:

```bash
# Clone the repository
git clone https://github.com/jax-protocol/jax-fs.git
cd jax-fs

# Build in release mode
cargo build --release

# Install to ~/.cargo/bin
cargo install --path crates/daemon

# Or run directly from the build directory
./target/release/jax --help
```

### Verify Installation

After installation, verify that `jax` is in your PATH:

```bash
jax --help
```

You should see output like:
```
A basic CLI example

Usage: jax [OPTIONS] <COMMAND>

Commands:
  bucket
  init
  daemon
  version
  help     Print this message or the help of the given subcommand(s)
```

If the command is not found, ensure `~/.cargo/bin` is in your PATH:

```bash
# Add to your shell profile (.bashrc, .zshrc, etc.)
export PATH="$HOME/.cargo/bin:$PATH"
```

## Initial Setup

### 1. Initialize Configuration

Create the configuration directory and generate your identity:

```bash
jax init
```

This creates:
- `~/.config/jax/` - Configuration directory (or custom path if specified with `--config-path`)
- `config.toml` - Daemon configuration
- `secret.pem` - Your Ed25519 identity keypair (keep this secure!)
- `jax.db` - SQLite database for bucket metadata
- `blobs/` - Directory for encrypted blob storage

**Security Note:** The `secret.pem` file contains your private key. Keep it secure and back it up safely. Anyone with access to this file can decrypt your buckets and impersonate you.

### 2. Configure Daemon (Optional)

The default configuration works out of the box, but you can customize settings by editing the generated `config.toml`:

```toml
[node]
# Path to your identity key
secret_key_path = "secret.pem"

# Path to blob storage
blobs_path = "blobs"

# Network bind port (0 = random ephemeral port)
bind_port = 0

[database]
# SQLite database path
path = "db.sqlite"

[http_server]
# API server listen address
api_addr = "127.0.0.1:3000"

# Web UI listen address
html_addr = "127.0.0.1:8080"
```

### 3. Start the Daemon

```bash
jax daemon
```

The daemon will:
- Start the HTTP API server on `http://localhost:3000`
- Start the Web UI server on `http://localhost:8080`
- Initialize the Iroh P2P node
- Begin listening for sync events
- Display your Node ID (public key)

Keep this running in a terminal, or run it as a background service (see below).

### Alternative: Gateway-Only Mode

For lightweight deployments that only need to serve published bucket content (no UI, no API):

```bash
jax daemon --gateway-only
```

The gateway mode provides:
- P2P peer syncing (mirror role)
- `/gw/:bucket_id/*path` endpoint for serving content with HTML file explorer
- `/_status/*` health endpoints
- Content negotiation (`Accept: application/json` for JSON responses)
- `?download=true` query param for raw file downloads

Use this when you need a minimal content server without the full daemon features.

### 4. Access the Web UI

Open your browser and navigate to:
```
http://localhost:8080
```

You should see the JaxBucket dashboard.

## Running as a Background Service

### Linux (systemd)

Create a systemd service file at `~/.config/systemd/user/jaxbucket.service`:

```ini
[Unit]
Description=JaxBucket P2P Storage Daemon
After=network.target

[Service]
Type=simple
ExecStart=%h/.cargo/bin/jax daemon
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=default.target
```

Enable and start the service:
```bash
systemctl --user enable jaxbucket
systemctl --user start jaxbucket

# Check status
systemctl --user status jaxbucket

# View logs
journalctl --user -u jaxbucket -f
```

### Linux (OpenRC / Gentoo)

Create an init script at `/etc/init.d/jax-daemon`:

```bash
#!/sbin/openrc-run

description="JaxBucket P2P Storage Daemon"

command="/home/YOUR_USERNAME/.cargo/bin/jax"
command_args="daemon"
command_user="YOUR_USERNAME:YOUR_USERNAME"
command_background=true
pidfile="/run/${RC_SVCNAME}.pid"

output_log="/var/log/${RC_SVCNAME}.log"
error_log="/var/log/${RC_SVCNAME}.err"

depend() {
    need net
    after firewall
}
```

Install and start the service:
```bash
# Make the script executable
chmod +x /etc/init.d/jax-daemon

# Add to default runlevel
rc-update add jax-daemon default

# Start the service
rc-service jax-daemon start

# Check status
rc-service jax-daemon status

# View logs
tail -f /var/log/jax-daemon.log
```

### macOS (launchd)

Create a launch agent at `~/Library/LaunchAgents/com.jaxbucket.daemon.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.jaxbucket.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>/Users/YOUR_USERNAME/.cargo/bin/jax</string>
        <string>daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/jaxbucket.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/jaxbucket.err</string>
</dict>
</plist>
```

Load the daemon:
```bash
launchctl load ~/Library/LaunchAgents/com.jaxbucket.daemon.plist

# Check status
launchctl list | grep jaxbucket

# View logs
tail -f /tmp/jaxbucket.log
```

## Troubleshooting

### "Command not found: jax"

Ensure `~/.cargo/bin` is in your PATH:
```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

### "Permission denied" on secret.pem

Fix file permissions:
```bash
chmod 600 ~/.config/jax/secret.pem
```

### "Database is locked"

Only one instance of `jax daemon` can run at a time. Stop any existing instances:
```bash
pkill -f "jax daemon"
```

### "Failed to bind address"

The HTTP port is already in use. Change it in `config.toml` or stop the conflicting service.

### FUSE: "Permission denied" or "Transport endpoint is not connected"

Ensure the FUSE kernel module is loaded and your user is in the `fuse` group:
```bash
# Load the FUSE module (if built as a module)
modprobe fuse

# Add your user to the fuse group
gpasswd -a YOUR_USERNAME fuse

# Log out and back in for the group change to take effect
```

On Gentoo, also verify your kernel config includes `CONFIG_FUSE_FS=y` or `CONFIG_FUSE_FS=m`. You can check with:
```bash
zgrep FUSE /proc/config.gz
```

### Reset Configuration

To start fresh:
```bash
# Backup first if needed
mv ~/.config/jax ~/.config/jax.backup

# Reinitialize
jax init
```

## Next Steps

- Read [concepts/](./concepts/) to understand how JaxBucket works internally
- Check [DEVELOPMENT.md](./DEVELOPMENT.md) for development and contribution guidelines

## Getting Help

- **Documentation**: https://docs.rs/jax-daemon
- **Issues**: https://github.com/jax-protocol/jax-fs/issues
- **Discussions**: https://github.com/jax-protocol/jax-fs/discussions
