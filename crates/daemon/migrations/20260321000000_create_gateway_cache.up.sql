CREATE TABLE gateway_cache (
    bucket_id        TEXT    NOT NULL,
    height           INTEGER NOT NULL,
    path             TEXT    NOT NULL,
    query_string     TEXT    NOT NULL DEFAULT '',
    link             TEXT    NOT NULL,
    content_size     INTEGER NOT NULL DEFAULT 0,
    mime_type        TEXT    NOT NULL DEFAULT 'application/octet-stream',
    created_at       INTEGER NOT NULL DEFAULT (unixepoch()),
    last_accessed    INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (bucket_id, height, path, query_string)
);

CREATE INDEX idx_cache_bucket_height ON gateway_cache (bucket_id, height);
CREATE INDEX idx_cache_last_accessed ON gateway_cache (last_accessed);
