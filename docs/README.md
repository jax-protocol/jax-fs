# jax-bucket

A peer-to-peer encrypted storage system built in Rust.

## What is jax-bucket?

jax-bucket is a decentralized file storage system that enables secure, encrypted data storage and synchronization between peers without relying on a central server. Think of it as a self-hosted, encrypted Dropbox where you control the infrastructure and the cryptography.

## Why jax-bucket?

**Problem**: Traditional cloud storage requires trusting a third party with your data. Even "encrypted" cloud services often hold the keys, meaning they can access your files if compelled (or compromised).

**Solution**: jax-bucket provides:

- **True end-to-end encryption**: Your keys never leave your device
- **Peer-to-peer sync**: No central server required
- **Content-addressed storage**: Deduplication and integrity verification built-in
- **Flexible access control**: Share with others without exposing your master key
- **Version history**: Every change creates a new version, nothing is lost

## Core Principles

1. **Zero-knowledge**: The system is designed so that even if someone has your encrypted data, they cannot read it without your keys
2. **Decentralized**: Peers connect directly using iroh's networking layer
3. **Content-addressed**: Data is stored by its hash, enabling deduplication and integrity verification
4. **Granular access**: Share individual buckets with specific peers using cryptographic key sharing

## Project Structure

```
jax-bucket/
├── crates/
│   ├── app/       # CLI binary and daemon (jax-bucket)
│   └── common/    # Shared library (jax-common)
├── docs/          # This documentation
├── agents/        # AI agent instructions
└── issues/        # Project tracking
```

## Getting Started

```bash
# Build the project
cargo build

# Initialize a new jax configuration
jax-bucket init

# Start the daemon
jax-bucket daemon

# Create a new bucket
jax-bucket bucket create my-bucket

# Add files
jax-bucket bucket add my-bucket /path/to/file.txt

# List contents
jax-bucket bucket ls my-bucket
```

## Documentation

- [Concepts](../agents/CONCEPTS.md) - High-level architecture and concepts
- [Usage](./usage.md) - CLI commands, binaries, and features

## Technology Stack

- **Rust** - Systems programming language
- **iroh** - P2P networking and blob storage
- **ChaCha20-Poly1305** - Content encryption
- **Ed25519** - Peer identity and authentication
- **X25519** - Key exchange for sharing
- **IPLD/DAG-CBOR** - Content-addressed data structures
