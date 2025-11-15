# Usage Guide

This guide covers how to use JaxBucket for creating encrypted storage buckets and syncing between peers.

## Prerequisites

Before using JaxBucket, make sure you have:

1. **Installed JaxBucket** - See [INSTALL.md](INSTALL.md)
2. **Initialized configuration** - Run `jax init`
3. **Started the daemon** - Run `jax daemon`

## CLI Overview

The `jax` CLI provides commands for managing buckets and interacting with the daemon:

```bash
jax [OPTIONS] <COMMAND>

Commands:
  bucket   # Bucket operations (create, list, add, ls, cat, clone, sync, share)
  daemon   # Start the JaxBucket daemon (HTTP server + P2P sync)
  init     # Initialize configuration
  version  # Show version information
```

**Global Options:**
- `--remote <URL>` - API endpoint (default: `http://localhost:3000`)
- `--config-path <PATH>` - Custom config directory (default: `~/.jax`)

## Core Commands

### Initialize Configuration

Before first use, initialize the JaxBucket configuration:

```bash
# Initialize with default settings
jax init

# Customize server addresses and ports
jax init --html-addr 0.0.0.0:8080 --api-addr 0.0.0.0:3000 --peer-port 9000
```

**Options:**
- `--html-addr <STRING>` - HTML server listen address (default: `0.0.0.0:8080`)
- `--api-addr <STRING>` - API server listen address (default: `0.0.0.0:3000`)
- `--peer-port <u16>` - P2P node listen port (default: ephemeral port)

This command creates:
- Configuration directory (`~/.jax` by default)
- SQLite database for metadata
- Keypair for P2P identity
- Blob storage directory
- Configuration file with server settings

### Start the Daemon

Start the JaxBucket daemon to run the HTTP server and P2P sync:

```bash
# Start with default settings
jax daemon

# Start with read-only web UI
jax daemon --ui-read-only

# Specify custom API hostname for web UI links
jax daemon --api-hostname http://myserver.local:3000
```

**Options:**
- `--ui-read-only` - Run the HTML UI in read-only mode (hides write operations)
- `--api-hostname <STRING>` - API hostname to use in HTML UI (default: `http://localhost:<api_port>`)

The daemon runs:
- HTTP API server (default: `http://localhost:3000`)
- Web UI server (default: `http://localhost:8080`)
- P2P sync node (Iroh-based networking)
- Background bucket synchronization

### Show Version

Display version information:

```bash
jax version
```

## Bucket Operations

### Bucket Identifiers

Most bucket commands accept either a bucket name or bucket ID:
- `--name <STRING>` - Use the human-readable bucket name
- `--bucket-id <UUID>` - Use the bucket's UUID directly

These options are mutually exclusive. Use `--name` for convenience, or `--bucket-id` when you need to reference a specific bucket by its UUID.

### Create a Bucket

Create a new encrypted bucket:

```bash
jax bucket create --name my-bucket
```

This creates a new bucket and returns its UUID.

### List Buckets

View all buckets:

```bash
jax bucket list

# Filter by name prefix
jax bucket list --prefix my-

# Limit number of results
jax bucket list --limit 10
```

**Options:**
- `--prefix <STRING>` - Filter buckets by name prefix
- `--limit <u32>` - Limit number of results

Returns a formatted text list of buckets with their names, IDs, and current link hashes.

### Add Files to a Bucket

Add a file to a bucket:

```bash
# Add a single file by bucket name
jax bucket add --name my-bucket --path /local/path/to/file.txt --mount-path /file.txt

# Add to a subdirectory in the bucket
jax bucket add --name my-bucket --path /local/path/to/file.txt --mount-path /docs/file.txt

# Add a file by bucket ID
jax bucket add --bucket-id a1b2c3d4-5678-90ab-cdef-1234567890ab --path /local/path/to/file.txt --mount-path /file.txt
```

**Options:**
- `--name <STRING>` or `--bucket-id <UUID>` - Bucket identifier (one required)
- `--path <STRING>` - Absolute path to file on local filesystem (required)
- `--mount-path <STRING>` - Path in bucket where file should be stored (required)

Files are automatically encrypted and stored in the bucket.

### List Bucket Contents

View the contents of a bucket:

```bash
# List root of bucket
jax bucket ls --name my-bucket

# List specific path in bucket
jax bucket ls --name my-bucket --path /docs

# List recursively (deep)
jax bucket ls --name my-bucket --deep

# Use bucket ID instead of name
jax bucket ls --bucket-id a1b2c3d4-5678-90ab-cdef-1234567890ab
```

**Options:**
- `--name <STRING>` or `--bucket-id <UUID>` - Bucket identifier (one required)
- `--path <STRING>` - Path in bucket to list (optional, defaults to root)
- `--deep` - List recursively (optional)

Shows the directory tree of the bucket with file paths, types, and link hashes.

### View File Contents

Download and view a file from a bucket:

```bash
# View a text file by bucket name
jax bucket cat --name my-bucket --path /path/in/bucket/file.txt

# View a file by bucket ID
jax bucket cat --bucket-id a1b2c3d4-5678-90ab-cdef-1234567890ab --path /file.txt
```

**Options:**
- `--name <STRING>` or `--bucket-id <UUID>` - Bucket identifier (one required)
- `--path <STRING>` - Path to file in bucket (required)

The file is decrypted and displayed. Text files are output directly; binary files are shown as hexadecimal.

### Share a Bucket

Share a bucket with another peer:

```bash
jax bucket share --bucket-id a1b2c3d4-5678-90ab-cdef-1234567890ab --peer-public-key <hex-encoded-public-key>
```

**Options:**
- `--bucket-id <UUID>` - Bucket ID to share (required)
- `--peer-public-key <STRING>` - Hex-encoded public key of the recipient peer (required)

The bucket's encryption key will be shared with the specified peer, allowing them to access the bucket.

## Web UI

The web interface provides a graphical way to interact with JaxBucket.

### Dashboard

Navigate to `http://localhost:8080` to see:
- List of all your buckets
- Bucket information (ID, name, size)
- Sync status
- Your Node ID

### Bucket Explorer

Click on a bucket to browse its contents:
- View directory structure
- Upload files
- Download files
- View file metadata

### File Viewer

Click on a file to:
- View file contents (for text files)
- Download the file
- See MIME type and metadata

## Working with Multiple Peers

### Get Your Node ID

Your Node ID is your public key that other peers use to share buckets with you:

```bash
# View in the web UI at http://localhost:8080
# Or check the service startup output when you run `jax service`
```

The Node ID is displayed in the format: `<hex-encoded-public-key>`

### Share a Bucket with Another Peer

1. **Get recipient's Node ID** (public key) from them (out-of-band, e.g., via email, QR code)
2. **Share the bucket:**
   ```bash
   jax bucket share --bucket-id <bucket-id> --peer-public-key <their-public-key>
   ```
3. **Recipient will automatically receive the bucket** on their next sync

### Sync Buckets

JaxBucket automatically syncs in the background, but you can also use the web UI to monitor sync status.

## Filesystem Clone & Sync

JaxBucket allows you to materialize encrypted buckets as regular filesystem directories. This enables you to work with bucket contents using standard file tools while maintaining sync state with the P2P network.

### Clone a Bucket to Filesystem

Clone an entire bucket to a local directory:

```bash
# Clone by bucket name
jax bucket clone --name my-bucket --directory ./my-local-copy

# Clone by bucket ID
jax bucket clone --bucket-id a1b2c3d4-5678-90ab-cdef-1234567890ab --directory /path/to/clone
```

**Options:**
- `--name <STRING>` or `--bucket-id <UUID>` - Bucket identifier (one required)
- `--directory <PATH>` - Target directory path (required, must be empty or non-existent)

**What happens:**
1. JaxBucket daemon exports the bucket from its local storage
2. All files are decrypted and written to the target directory
3. A hidden `.jax/` directory is created to track sync state
4. You can now browse and use the files with any tool

**Example:**
```bash
$ jax bucket clone --name photos --directory ~/photos-clone
Cloned bucket 'photos' (a1b2c3d4-...) to /Users/you/photos-clone
Exported 42 files

$ ls ~/photos-clone/
vacation/  family/  .jax/

$ ls ~/photos-clone/vacation/
beach.jpg  sunset.jpg  hotel.jpg
```

**Requirements:**
- Target directory must be empty or not exist
- Daemon must be running (`jax daemon`)
- Bucket must already be synced to your daemon

### Sync Updates from Network

Pull the latest changes from the network to your cloned directory:

```bash
jax bucket sync --directory ./my-local-copy
```

**What happens:**
1. Reads `.jax/config.json` to identify the bucket
2. Queries daemon for current bucket state
3. Compares with last synced state
4. If updates exist: overwrites changed files, adds new files, removes deleted files
5. Updates `.jax/` tracking state

**Example:**
```bash
$ jax bucket sync --directory ~/photos-clone
Synced bucket 'photos' from height 5 to height 8
Updated 7 files

# If already up to date:
$ jax bucket sync --directory ~/photos-clone
Bucket 'photos' is already up to date (height: 8)
```

### How Clone/Sync Works

#### Architecture

```
┌──────────────┐
│   jax CLI    │
└──────┬───────┘
       │ HTTP (localhost:3000)
       ↓
┌────────────────────────────────┐
│  jax daemon                    │
│  ┌────────────┐  ┌───────────┐│
│  │ Local Peer │→│ Blob Store││
│  └────────────┘  └───────────┘│
│         ↓ (already synced)    │
│  ┌─────────────────────┐      │
│  │ P2P Network (Iroh)  │      │
│  └─────────────────────┘      │
└───────────────┬────────────────┘
                │ export
                ↓
         ┌──────────────┐
         │  Filesystem  │
         │  ./clone/    │
         └──────────────┘
```

**Key Points:**
- Clone/sync use the daemon's local peer storage (no extra P2P traffic)
- Daemon handles P2P sync separately (background process)
- Clone just "materializes" what the daemon already has
- Data duplication is intentional (blob store + filesystem)

#### The `.jax` Directory

Every cloned bucket contains a hidden `.jax/` directory:

```
./my-clone/
├── .jax/
│   ├── config.json      # Bucket metadata and sync state
│   └── hashes.json      # File path to hash mapping
├── file1.txt
└── folder/
    └── file2.txt
```

**`config.json` format:**
```json
{
  "bucket_id": "a1b2c3d4-5678-90ab-cdef-1234567890ab",
  "bucket_name": "photos",
  "last_synced_link": {
    "codec": 113,
    "hash": "bafk..."
  },
  "last_synced_height": 8
}
```

**Purpose:**
- Tracks which bucket this directory represents
- Records last synced state (height/link) to detect updates
- Enables future two-way sync (detect local changes)

**`hashes.json` format:**
```json
{
  "entries": {
    "file1.txt": [
      "bafk...",                    // blob hash (CID)
      [123, 45, 67, ...32 bytes...] // BLAKE3 hash of plaintext
    ],
    "folder/file2.txt": [
      "bafk...",
      [89, 12, 34, ...32 bytes...]
    ]
  }
}
```

**Purpose:**
- Maps each file path to its blob hash and plaintext hash
- Enables efficient change detection without re-hashing files
- Foundation for future two-way sync

### Hash Prepending Feature

JaxBucket uses a special encryption format that embeds content hashes:

**Standard encrypted blob format:**
```
[ nonce(12) ][ ciphertext ][ auth_tag(16) ]
```

**JaxBucket format:**
```
[ nonce(12) ][ encrypted( hash(32) || plaintext ) ][ auth_tag(16) ]
                          ↑
                BLAKE3 hash of unencrypted content
```

**Benefits:**
1. **Integrity Verification**: Hash is checked on every decrypt (data corruption detection)
2. **Efficient Change Detection**: Can extract hash without full decryption
3. **Deduplication**: Same content = same hash (even if encrypted separately)
4. **Future Sync Optimization**: Compare hashes to detect local vs remote changes

**Example usage in export:**
```rust
// Get encrypted blob from store
let encrypted = blobs.get(&hash).await?;

// Extract hash WITHOUT decrypting entire file
let plaintext_hash = secret.extract_plaintext_hash(&encrypted)?;

// Decrypt and write (hash verified automatically)
let plaintext = secret.decrypt(&encrypted)?;
fs::write(path, plaintext)?;
```

### Use Cases

**Personal File Sync:**
```bash
# On computer A
jax bucket create --name documents
jax bucket add --name documents --path ~/Documents --mount-path /

# On computer B (after bucket syncs via P2P)
jax bucket clone --name documents --directory ~/Documents-sync
```

**Backup Workflow:**
```bash
# Clone important bucket to external drive
jax bucket clone --name family-photos --directory /Volumes/Backup/photos

# Later, update the backup
jax bucket sync --directory /Volumes/Backup/photos
```

**Collaborative Editing:**
```bash
# Alice clones shared project
jax bucket clone --name team-project --directory ~/project

# Work on files with any editor
vim ~/project/README.md

# Later, pull updates from team (Bob added files via daemon)
jax bucket sync --directory ~/project
# See Bob's changes!
```

### Limitations & Future Work

**Current limitations:**
- **One-way sync only**: Changes to cloned filesystem are not pushed back
- **Full directory replacement**: Sync overwrites entire directory
- **No conflict resolution**: Last sync wins
- **No selective sync**: Always syncs entire bucket

**Planned features:**
- Two-way sync (detect local changes and push to daemon)
- Selective path sync (only sync specific subdirectories)
- Conflict resolution (merge remote + local changes)
- Live filesystem watching (auto-sync on changes)
- Incremental sync (only update changed files)

### Troubleshooting

**"Directory already initialized as a clone"**
- Directory already contains `.jax/`
- Either use `sync` instead of `clone`, or clone to a different directory

**"Directory already exists and is not empty"**
- Target directory must be empty for clone
- Move existing files or choose a different directory

**"Directory is not a cloned bucket"**
- No `.jax/` directory found
- Make sure you're running `sync` on a previously cloned directory

**"Bucket not found"**
- Daemon doesn't have this bucket
- Wait for P2P sync to complete, or verify bucket name/ID
