DROP INDEX IF EXISTS idx_bucket_log_bucket_published;
ALTER TABLE bucket_log DROP COLUMN published;
