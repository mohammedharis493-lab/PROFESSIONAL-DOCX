CREATE TABLE recent_searches (
    normalized_query TEXT PRIMARY KEY NOT NULL,
    query_text TEXT NOT NULL,
    last_used_at_ms INTEGER NOT NULL,
    use_count INTEGER NOT NULL DEFAULT 1 CHECK (use_count >= 1)
);

CREATE INDEX idx_recent_searches_last_used
    ON recent_searches(last_used_at_ms DESC);
