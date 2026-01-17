# JaxBucket

[![Crates.io](https://img.shields.io/crates/v/jax-bucket.svg)](https://crates.io/crates/jax-bucket)
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

## Overview

JaxBucket is a local-first, encrypted storage system built on [Iroh](https://iroh.computer/). It provides content-addressed, encrypted file storage with automatic peer-to-peer synchronization between authorized devices.

## Features

- **End-to-End Encryption**: All files encrypted with ChaCha20-Poly1305
- **P2P Sync**: Automatic synchronization via Iroh's networking stack
- **Content-Addressed**: Files stored as immutable, hash-linked DAGs
- **Cryptographic Access Control**: ECDH + AES Key Wrap for secure multi-device access
- **Local-First**: Works offline, syncs when connected

## Quick Start

```bash
cargo install jax-bucket

jax init
jax daemon
jax bucket create my-bucket
jax bucket add <bucket-id> ./file.txt
jax bucket ls <bucket-id>
```

## Crates

| Crate | Description |
|-------|-------------|
| [jax-bucket](crates/app/) | CLI and daemon binary |
| [jax-common](crates/common/) | Core library (crypto, mount, peer) |

## Documentation

- [CLI Usage](crates/app/README.md) - Commands and API reference
- [Library API](crates/common/README.md) - Core data structures
- [Architecture](agents/CONCEPTS.md) - System design and concepts

## Contributing

See [agents/CONTRIBUTING.md](agents/CONTRIBUTING.md) for guidelines.

## License

MIT - see [LICENSE](LICENSE)

## Built With

- [Iroh](https://iroh.computer/) - P2P networking
- [Rust](https://www.rust-lang.org/) - Systems programming
- [DAG-CBOR](https://ipld.io/) - Content-addressed serialization
