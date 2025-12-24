-- Blob metadata table for MinIO+SQLite store
-- All blob data is stored in MinIO; this table only tracks metadata
CREATE TABLE IF NOT EXISTS blobs (
    hash TEXT PRIMARY KEY,              -- base32-encoded BLAKE3 hash
    size INTEGER NOT NULL,              -- blob size in bytes
    has_outboard INTEGER NOT NULL,      -- 1 if blob has outboard (size > 16KB), 0 otherwise
    state TEXT NOT NULL,                -- 'complete' or 'partial'
    created_at INTEGER NOT NULL,        -- unix timestamp
    updated_at INTEGER NOT NULL         -- unix timestamp
);

-- Tags table for named references to blobs
CREATE TABLE IF NOT EXISTS tags (
    name TEXT PRIMARY KEY,              -- tag name
    hash TEXT NOT NULL,                 -- base32-encoded hash
    format TEXT NOT NULL,               -- 'raw' or 'hash_seq'
    created_at INTEGER NOT NULL         -- unix timestamp
);

-- Index for looking up tags by hash
CREATE INDEX IF NOT EXISTS idx_tags_hash ON tags(hash);

-- Index for listing blobs by state (useful for finding partials)
CREATE INDEX IF NOT EXISTS idx_blobs_state ON blobs(state);
