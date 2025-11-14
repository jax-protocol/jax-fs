-- Drop bucket_log table and indexes
DROP INDEX IF EXISTS idx_bucket_log_bucket_link;
DROP INDEX IF EXISTS idx_bucket_log_bucket_height;
DROP TABLE IF EXISTS bucket_log;
