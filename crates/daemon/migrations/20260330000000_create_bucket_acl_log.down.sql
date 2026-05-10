DROP TABLE IF EXISTS bucket_acl_log;

CREATE TABLE bucket_status (
    bucket_id TEXT PRIMARY KEY,
    status TEXT NOT NULL DEFAULT 'pending',
    shared_by TEXT,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_bucket_status_status ON bucket_status(status);
