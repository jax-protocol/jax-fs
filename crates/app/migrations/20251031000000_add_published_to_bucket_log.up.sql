-- Add published column to track publication status at each version
ALTER TABLE bucket_log ADD COLUMN published BOOLEAN NOT NULL DEFAULT FALSE;

-- Index for efficient lookup of latest published version
CREATE INDEX idx_bucket_log_bucket_published ON bucket_log(bucket_id, published, height DESC);
