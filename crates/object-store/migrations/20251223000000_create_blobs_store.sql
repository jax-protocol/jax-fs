-- Create blobs metadata table
CREATE TABLE IF NOT EXISTS blobs (
    hash TEXT PRIMARY KEY,
    size INTEGER NOT NULL,
    has_outboard INTEGER NOT NULL DEFAULT 0,
    state TEXT NOT NULL DEFAULT 'complete',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Index for querying by state (e.g., incomplete uploads)
CREATE INDEX IF NOT EXISTS idx_blobs_state ON blobs(state);
