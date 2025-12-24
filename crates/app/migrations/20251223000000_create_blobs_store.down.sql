-- Rollback: drop blobs store tables
DROP INDEX IF EXISTS idx_blobs_state;
DROP INDEX IF EXISTS idx_tags_hash;
DROP TABLE IF EXISTS tags;
DROP TABLE IF EXISTS blobs;
