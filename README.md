# JaxBucket
<!-- test: verify daemon spawn for jax-fs -->

[![Crates.io](https://img.shields.io/crates/v/jax-daemon.svg)](https://crates.io/crates/jax-daemon)
[![Documentation](https://docs.rs/jax-common/badge.svg)](https://docs.rs/jax-common)
[![CI](https://github.com/jax-protocol/jax-buckets/actions/workflows/ci.yml/badge.svg)](https://github.com/jax-protocol/jax-buckets/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**End-to-End Encrypted Storage Buckets with Peer-to-Peer Synchronization**

> **SECURITY DISCLAIMER**
>
> **This software has NOT been audited by security professionals and is NOT production-ready.**
>
> JaxBucket is an experimental project built for learning and demonstration purposes. The cryptographic implementation and protocol design have not undergone formal security review. Do not use this software to protect sensitive, confidential, or production data.
>
> Use at your own risk. The authors assume no liability for data loss, security breaches, or other issues arising from the use of this software.

## Desktop App

Download the latest Jax Desktop:

- **[macOS (Apple Silicon)](https://github.com/jax-protocol/jax-fs/releases/latest/download/Jax_aarch64.dmg)**
- **[Linux (.AppImage)](https://github.com/jax-protocol/jax-fs/releases/latest/download/jax-desktop_amd64.AppImage)**
- **[Linux (.deb)](https://github.com/jax-protocol/jax-fs/releases/latest/download/jax-desktop_amd64.deb)**

Or browse [all releases](https://github.com/jax-protocol/jax-fs/releases).

## Overview

JaxBucket is a local-first, encrypted storage system built on [Iroh](https://iroh.computer/). It provides content-addressed, encrypted file storage with automatic peer-to-peer synchronization between authorized devices.

## Features

- **End-to-End Encryption**: All files encrypted with ChaCha20-Poly1305
- **P2P Sync**: Automatic synchronization via Iroh's networking stack
- **Content-Addressed**: Files stored as immutable, hash-linked DAGs
- **Cryptographic Access Control**: ECDH + AES Key Wrap for secure multi-device access
- **Local-First**: Works offline, syncs when connected

## Downloads

Pre-built desktop binaries are available on the [GitHub Releases](https://github.com/jax-protocol/jax-fs/releases) page:

| Platform | File | FUSE Mount Support |
|----------|------|--------------------|
| macOS (Apple Silicon) | `Jax_*_aarch64.dmg` | No |
| macOS (Apple Silicon + FUSE) | `Jax_*_aarch64_fuse.dmg` | Yes (requires [macFUSE](https://osxfuse.github.io/)) |
| Linux (Debian/Ubuntu) | `jax-desktop_*_amd64.deb` | No |
| Linux (portable) | `jax-desktop_*_amd64.AppImage` | No |

The `_fuse` variant enables mounting buckets as local filesystem directories. See the [Installation Guide](agents/INSTALL.md) for details.

## Quick Start

```bash
# Install (or update) the CLI
curl -fsSL https://raw.githubusercontent.com/jax-protocol/jax-fs/main/install.sh | sh

# Or install via cargo
cargo install jax-daemon

jax init
jax daemon
jax bucket create my-bucket
jax bucket add <bucket-id> ./file.txt
jax bucket ls <bucket-id>
```

## Crates

| Crate | Description |
|-------|-------------|
| [jax-daemon](crates/daemon/) | CLI and daemon binary |
| [jax-common](crates/common/) | Core library (crypto, mount, peer) |
| [jax-desktop](crates/desktop/) | Tauri desktop app |

## Documentation

- [CLI Usage](crates/daemon/README.md) - Commands and API reference
- [Library API](crates/common/README.md) - Core data structures
- [Architecture](agents/concepts/) - System design and concepts
- [Installation](agents/INSTALL.md) - Installation and setup guide
- [Development](agents/DEVELOPMENT.md) - Development environment setup

## Contributing

See [agents/CONTRIBUTING.md](agents/CONTRIBUTING.md) for guidelines.

## License

MIT - see [LICENSE](LICENSE)

## Built With

- [Iroh](https://iroh.computer/) - P2P networking
- [Rust](https://www.rust-lang.org/) - Systems programming
- [DAG-CBOR](https://ipld.io/) - Content-addressed serialization
