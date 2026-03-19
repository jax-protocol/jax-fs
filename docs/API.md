# API Reference

This document describes the HTTP API endpoints for jax-daemon.

## Overview

jax-daemon runs two servers on separate ports:
- **API Server**: REST API for bucket operations (private, localhost only)
- **Gateway Server**: Read-only content serving (public-facing)

## Base URLs

When using the dev environment (`./bin/dev`):

| Node | API Server | Gateway |
|------|------------|---------|
| owner | http://localhost:5002 | http://localhost:8081 |
| _owner | http://localhost:5003 | http://localhost:8082 |
| mirror | http://localhost:5004 | http://localhost:8083 |

Default production ports: API on 5001, Gateway on 8080.

## Health Endpoints

All servers expose health endpoints at `/_status/`:

### GET /_status/livez
Liveness check - returns immediately if server is running.

```bash
curl http://localhost:5001/_status/livez
```

Response: `{"status": "ok"}`

### GET /_status/readyz
Readiness check - verifies all dependencies are ready.

```bash
curl http://localhost:5001/_status/readyz
```

Response: `{"status": "ok"}` or `{"status": "error", "message": "..."}`

### GET /_status/identity
Returns the node's peer identity.

```bash
curl http://localhost:5001/_status/identity
```

Response:
```json
{
  "node_id": "2gx...abc"
}
```

### GET /_status/version
Returns build version information.

```bash
curl http://localhost:5001/_status/version
```

## Bucket API

All bucket operations are under `/api/v0/bucket/`. Most use POST with JSON bodies.

### POST /api/v0/bucket - Create Bucket

Creates a new bucket.

```bash
curl -X POST http://localhost:5001/api/v0/bucket \
  -H "Content-Type: application/json" \
  -d '{"name": "my-bucket"}'
```

Request:
```json
{
  "name": "my-bucket"
}
```

Response (201 Created):
```json
{
  "bucket_id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "my-bucket",
  "created_at": "2024-01-20T12:00:00Z"
}
```

### POST /api/v0/bucket/list - List Buckets

Lists all buckets on the node.

```bash
curl -X POST http://localhost:5001/api/v0/bucket/list \
  -H "Content-Type: application/json" \
  -d '{}'
```

Request:
```json
{
  "prefix": "optional-filter",
  "limit": 100
}
```

Response:
```json
{
  "buckets": [
    {
      "bucket_id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "my-bucket",
      "link": { "codec": 85, "hash": "..." },
      "created_at": "2024-01-20T12:00:00Z"
    }
  ]
}
```

### POST /api/v0/bucket/ls - List Directory

Lists contents of a directory within a bucket.

```bash
curl -X POST http://localhost:5001/api/v0/bucket/ls \
  -H "Content-Type: application/json" \
  -d '{"bucket_id": "550e8400-...", "path": "/"}'
```

Request:
```json
{
  "bucket_id": "550e8400-e29b-41d4-a716-446655440000",
  "path": "/",
  "deep": false
}
```

Response:
```json
{
  "items": [
    {
      "path": "/readme.txt",
      "name": "readme.txt",
      "link": { "codec": 85, "hash": "..." },
      "is_dir": false,
      "mime_type": "text/plain"
    },
    {
      "path": "/docs",
      "name": "docs",
      "link": { "codec": 85, "hash": "..." },
      "is_dir": true,
      "mime_type": "inode/directory"
    }
  ]
}
```

### POST /api/v0/bucket/cat - Read File (JSON)

Reads file content, returns base64-encoded.

```bash
curl -X POST http://localhost:5001/api/v0/bucket/cat \
  -H "Content-Type: application/json" \
  -d '{"bucket_id": "550e8400-...", "path": "/readme.txt"}'
```

Request:
```json
{
  "bucket_id": "550e8400-e29b-41d4-a716-446655440000",
  "path": "/readme.txt",
  "at": "optional-hash-for-specific-version"
}
```

Response:
```json
{
  "path": "/readme.txt",
  "content": "SGVsbG8gV29ybGQh",
  "size": 12,
  "mime_type": "text/plain"
}
```

### GET /api/v0/bucket/cat - Read File (Binary)

Returns raw file content with proper Content-Type.

```bash
curl "http://localhost:5001/api/v0/bucket/cat?bucket_id=550e8400-...&path=/readme.txt"
```

Query params:
- `bucket_id` (required): UUID of the bucket
- `path` (required): Absolute path to file
- `at` (optional): Version hash
- `download` (optional): If `true`, forces download (attachment disposition)

### POST /api/v0/bucket/add - Upload File

Uploads files using multipart form data.

```bash
curl -X POST http://localhost:5001/api/v0/bucket/add \
  -F "bucket_id=550e8400-..." \
  -F "mount_path=/" \
  -F "file=@local-file.txt"
```

Form fields:
- `bucket_id`: UUID of the bucket
- `mount_path`: Directory path to upload into (e.g., `/` or `/docs`)
- `file` or `files`: File(s) to upload (can be multiple)

Response:
```json
{
  "bucket_link": { "codec": 85, "hash": "..." },
  "files": [
    {
      "mount_path": "/local-file.txt",
      "mime_type": "text/plain",
      "size": 1234,
      "success": true,
      "error": null
    }
  ],
  "total_files": 1,
  "successful_files": 1,
  "failed_files": 0
}
```

### POST /api/v0/bucket/mkdir - Create Directory

Creates a directory within a bucket.

```bash
curl -X POST http://localhost:5001/api/v0/bucket/mkdir \
  -H "Content-Type: application/json" \
  -d '{"bucket_id": "550e8400-...", "path": "/new-folder"}'
```

Request:
```json
{
  "bucket_id": "550e8400-e29b-41d4-a716-446655440000",
  "path": "/new-folder"
}
```

### POST /api/v0/bucket/delete - Delete File/Directory

Deletes a file or directory from a bucket.

```bash
curl -X POST http://localhost:5001/api/v0/bucket/delete \
  -H "Content-Type: application/json" \
  -d '{"bucket_id": "550e8400-...", "path": "/old-file.txt"}'
```

Request:
```json
{
  "bucket_id": "550e8400-e29b-41d4-a716-446655440000",
  "path": "/old-file.txt"
}
```

### POST /api/v0/bucket/mv - Move/Rename

Moves or renames a file or directory.

```bash
curl -X POST http://localhost:5001/api/v0/bucket/mv \
  -H "Content-Type: application/json" \
  -d '{"bucket_id": "550e8400-...", "from": "/old.txt", "to": "/new.txt"}'
```

### POST /api/v0/bucket/rename - Rename Bucket

Renames a bucket.

```bash
curl -X POST http://localhost:5001/api/v0/bucket/rename \
  -H "Content-Type: application/json" \
  -d '{"bucket_id": "550e8400-...", "name": "new-name"}'
```

### POST /api/v0/bucket/share - Create Share Link

Creates a shareable link for a bucket (read-only access).

```bash
curl -X POST http://localhost:5001/api/v0/bucket/share \
  -H "Content-Type: application/json" \
  -d '{"bucket_id": "550e8400-..."}'
```

### POST /api/v0/bucket/ping - Sync with Peer

Initiates sync with a remote peer for a bucket.

```bash
curl -X POST http://localhost:5001/api/v0/bucket/ping \
  -H "Content-Type: application/json" \
  -d '{"bucket_id": "550e8400-...", "node_id": "2gx..."}'
```

### POST /api/v0/bucket/export - Export Bucket

Exports bucket contents.

## Gateway Endpoints

The gateway server provides read-only access to bucket contents:

### GET /gw/:bucket_id/*file_path

Serves files from a bucket. The bucket_id can be either:
- A UUID for owned buckets
- A share token for shared buckets

```bash
# Using dev API helper (recommended)
./bin/dev api gw fetch 550e8400-... /           # List root directory (JSON)
./bin/dev api gw fetch 550e8400-... /docs/      # List subdirectory
./bin/dev api full fetch 550e8400-... /file.txt # Fetch file content

# Direct curl (if needed)
curl http://localhost:8080/gw/550e8400-.../
curl http://localhost:8080/gw/550e8400-.../path/to/file.txt
```

Query parameters:
- `download=true` - Force download with Content-Disposition: attachment
- `view=true` - Show file in viewer UI instead of rendering HTML/Markdown
- `deep=true` - Recursively list all files (for directories)
