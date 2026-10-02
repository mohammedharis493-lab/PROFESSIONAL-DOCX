CREATE TABLE document_pins (
    document_id TEXT PRIMARY KEY NOT NULL,
    pinned_at_ms INTEGER NOT NULL,
    FOREIGN KEY (document_id)
        REFERENCES documents(document_id)
        ON DELETE CASCADE
);

CREATE INDEX idx_document_pins_pinned_at
    ON document_pins(pinned_at_ms DESC);

CREATE TABLE recent_document_access (
    document_id TEXT PRIMARY KEY NOT NULL,
    last_file_instance_id TEXT,
    last_opened_at_ms INTEGER NOT NULL,
    open_count INTEGER NOT NULL DEFAULT 1 CHECK (open_count >= 1),
    FOREIGN KEY (document_id)
        REFERENCES documents(document_id)
        ON DELETE CASCADE,
    FOREIGN KEY (last_file_instance_id)
        REFERENCES file_instances(file_instance_id)
        ON DELETE SET NULL
);

CREATE INDEX idx_recent_document_access_last_opened
    ON recent_document_access(last_opened_at_ms DESC);
