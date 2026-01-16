-- Create fuse_mounts table for persistent FUSE mount configurations
CREATE TABLE fuse_mounts (
    -- Unique identifier for this mount configuration
    mount_id TEXT PRIMARY KEY,
    -- The bucket to mount
    bucket_id TEXT NOT NULL,
    -- The local filesystem path to mount at
    mount_point TEXT NOT NULL UNIQUE,
    -- Whether this mount is enabled
    enabled INTEGER NOT NULL DEFAULT 1,
    -- Whether to auto-mount on daemon startup
    auto_mount INTEGER NOT NULL DEFAULT 0,
    -- Mount in read-only mode
    read_only INTEGER NOT NULL DEFAULT 0,
    -- Cache size in MB (default 100)
    cache_size_mb INTEGER NOT NULL DEFAULT 100,
    -- Cache TTL in seconds (default 60)
    cache_ttl_secs INTEGER NOT NULL DEFAULT 60,
    -- PID of the mount process when running
    pid INTEGER,
    -- Current status: stopped, running, error
    status TEXT NOT NULL DEFAULT 'stopped',
    -- Error message if status is 'error'
    error_message TEXT,
    -- When this mount was created
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    -- When this mount was last updated
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Index for efficient queries by bucket_id
CREATE INDEX idx_fuse_mounts_bucket ON fuse_mounts(bucket_id);

-- Index for finding auto-mount entries on startup
CREATE INDEX idx_fuse_mounts_auto ON fuse_mounts(auto_mount) WHERE auto_mount = 1;
