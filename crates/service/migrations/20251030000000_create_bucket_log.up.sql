-- Create bucket_log table for tracking state transitions
CREATE TABLE bucket_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- The UUID of the bucket this log entry belongs to
    bucket_id TEXT NOT NULL,
    -- The friendly name of the bucket at this point in time
    name TEXT NOT NULL,
    -- The current link at this log entry (stored as base32 CID string)
    current_link VARCHAR(255) NOT NULL,
    -- The previous link (null for genesis)
    previous_link VARCHAR(255),
    -- The height of this entry in the log chain
    height INTEGER NOT NULL,
    -- When this log entry was created
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

    -- Ensure one entry per height per bucket
    UNIQUE(bucket_id, height),
    -- Ensure one entry per link per bucket
    UNIQUE(bucket_id, current_link)
);

-- Index for efficient queries by bucket and height
CREATE INDEX idx_bucket_log_bucket_height ON bucket_log(bucket_id, height DESC);

-- Index for efficient queries by bucket and link
CREATE INDEX idx_bucket_log_bucket_link ON bucket_log(bucket_id, current_link);
