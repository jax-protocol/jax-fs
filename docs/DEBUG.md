# Debugging Workflow

Guide for debugging jax-bucket during development.

## Quick Start

```bash
# Start the dev environment (3 nodes in tmux)
# Automatically applies fixtures (creates demo buckets)
./bin/dev

# In another terminal - test the API
./bin/dev api owner health   # Should return {"status": "ok"}
./bin/dev api owner list     # List buckets

# View logs
./bin/dev logs              # Tail all node logs
./bin/dev logs grep ERROR   # Search for errors
```

## Dev Environment

The `./bin/dev` script reads node configuration from `./bin/dev_/nodes.toml`:

| Node | Nick | Role | API Port | Gateway Port | Blob Store |
|------|------|------|----------|-------------|------------|
| node0 | owner | Primary owner | 5002 | 8081 | legacy |
| node1 | _owner | Replica owner | 5003 | 8082 | filesystem |
| node2 | mirror | Mirror | 5004 | 8083 | s3 (MinIO) |

Each node runs both an API server (private, mutation/RPC) and a gateway server (public, read-only) on separate ports.

### Commands

```bash
./bin/dev          # Start all nodes in tmux
./bin/dev clean    # Remove all data (fresh start)
./bin/dev kill     # Kill the tmux session
./bin/dev status   # Show node status and health
```

### Tmux Navigation

Once in the tmux session:
- `Ctrl+B` then `0/1/2` - Switch to pane 0, 1, or 2
- `Ctrl+B` then arrow keys - Navigate between panes
- `Ctrl+B` then `d` - Detach from session
- `tmux attach -t jax-dev` - Reattach to session

## Log Files

Logs are written to `./data/<node>/logs/jax.log.YYYY-MM-DD`.

### Viewing Logs

```bash
# Tail all logs in real-time
./bin/logs

# Tail specific node
./bin/logs node0

# Search logs
./bin/logs grep "bucket"
./bin/logs grep ERROR

# Show latest log file
./bin/logs cat node0

# List all log files
./bin/logs list
```

### Log Format

Logs use the tracing format:
```
2024-01-20T12:00:00.123456Z  INFO http_server: API server listening addr=0.0.0.0:5002
2024-01-20T12:00:01.456789Z DEBUG mount: Adding file path="/test.txt" size=123
```

Key fields:
- Timestamp (UTC)
- Level: ERROR, WARN, INFO, DEBUG, TRACE
- Target module
- Message and structured fields

### Filtering by Level

Set `RUST_LOG` environment variable:
```bash
RUST_LOG=debug cargo run ...     # Debug and above
RUST_LOG=warn cargo run ...      # Warn and above
RUST_LOG=jax=debug cargo run ... # Debug for jax, info for deps
```

## API Testing

Use `./bin/dev api` for quick API tests:

```bash
# Health checks (all nodes)
./bin/dev api owner health         # Liveness on owner node
./bin/dev api _owner ready         # Readiness on replica
./bin/dev api mirror identity      # Mirror identity

# Bucket operations (all nodes)
./bin/dev api owner create "test"       # Create bucket
./bin/dev api owner list                # List all buckets
./bin/dev api _owner ls <bucket_id> /   # List root directory
./bin/dev api owner cat <bucket_id> /f  # Read file
./bin/dev api owner upload <bucket_id> / ./file.txt
```

## Common Debugging Scenarios

### Server Won't Start

1. Check if ports are in use:
   ```bash
   lsof -i :5002
   lsof -i :8081
   ```

2. Kill existing sessions:
   ```bash
   ./bin/dev kill
   ```

3. Check logs for startup errors:
   ```bash
   ./bin/dev logs grep "error\|Error\|ERROR"
   ```

### API Returns Error

1. Check the request:
   ```bash
   ./bin/dev api owner list  # Returns JSON with error
   ```

2. Check logs for the error:
   ```bash
   ./bin/dev logs grep ERROR
   ```

3. Look for the specific handler:
   ```bash
   ./bin/dev logs grep "bucket/list"
   ```

### Bucket Operations Fail

1. Check bucket exists:
   ```bash
   ./bin/dev api owner list
   ```

2. Check logs for mount errors:
   ```bash
   ./bin/dev logs grep "mount\|Mount"
   ```

### Peer Sync Issues

1. Check node identities:
   ```bash
   ./bin/dev api owner identity
   ./bin/dev api _owner identity
   ```

2. Look for peer connection logs:
   ```bash
   ./bin/dev logs grep "peer\|connect"
   ```

### MinIO (S3) Issues

1. Check MinIO is running:
   ```bash
   ./bin/minio status
   ```

2. View MinIO console: http://localhost:9001
   - Username: minioadmin
   - Password: minioadmin

3. Check S3 connection logs:
   ```bash
   ./bin/dev logs grep "s3\|S3\|minio"
   ```

## Using Claude for Debugging

When asking Claude for help debugging, provide:

1. **Log excerpt**: Run `./bin/logs grep ERROR` and share relevant lines
2. **Command that failed**: What you ran and the error message
3. **Node configuration**: Which node (node0/1/2) you were using
4. **Recent changes**: Any code changes you made

Example prompt:
```
I'm seeing this error when creating a bucket:

$ ./bin/dev api owner create "test"
{"error": "Mount error: ..."}

Logs show:
2024-01-20T12:00:00Z ERROR create: Failed to initialize mount: ...

Can you help debug this?
```

## Database Inspection

SQLite databases are at `./data/<node>/jax.db`.

```bash
# Use the db helper
./bin/db node0 "SELECT * FROM buckets LIMIT 5"
./bin/db node0 ".tables"
./bin/db node0 ".schema buckets"
```

## Performance Profiling

For performance issues:

1. Enable trace logging:
   ```bash
   RUST_LOG=trace ./bin/dev
   ```

2. Look for slow operations in logs:
   ```bash
   ./bin/logs grep -E "[0-9]{4,}ms"
   ```

## Cleanup

```bash
# Kill tmux session
./bin/dev kill

# Remove all data (fresh start)
./bin/dev clean

# Stop MinIO
./bin/minio down
```
