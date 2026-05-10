-- Migrate existing bucket_status rows to bucket_acl_log events
-- before dropping the table.

CREATE TABLE bucket_acl_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    bucket_id TEXT NOT NULL,
    event TEXT NOT NULL,
    actor TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_bucket_acl_log_bucket_id ON bucket_acl_log(bucket_id);

-- Migrate existing data: map old status values to ACL events
INSERT INTO bucket_acl_log (bucket_id, event, actor, created_at)
SELECT bucket_id,
       CASE status
           WHEN 'active' THEN 'approved'
           WHEN 'pending' THEN 'shared'
           WHEN 'ignored' THEN 'ignored'
       END,
       COALESCE(shared_by, 'self'),
       created_at
FROM bucket_status;

DROP TABLE IF EXISTS bucket_status;
