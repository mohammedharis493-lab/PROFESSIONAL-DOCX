CREATE TABLE document_relationships (
    document_relationship_id TEXT PRIMARY KEY NOT NULL,
    source_document_id TEXT NOT NULL REFERENCES documents(document_id),
    target_document_id TEXT NOT NULL REFERENCES documents(document_id),
    relationship_type TEXT NOT NULL CHECK (
        length(trim(relationship_type)) BETWEEN 1 AND 80
    ),
    created_at_ms INTEGER NOT NULL,
    created_by TEXT,
    removed_at_ms INTEGER,
    removed_by TEXT,
    CHECK (source_document_id <> target_document_id),
    CHECK (removed_at_ms IS NULL OR removed_at_ms >= created_at_ms)
);

CREATE UNIQUE INDEX idx_document_relationships_active_unique
    ON document_relationships(
        source_document_id,
        target_document_id,
        relationship_type COLLATE NOCASE
    )
    WHERE removed_at_ms IS NULL;

CREATE INDEX idx_document_relationships_source_active
    ON document_relationships(source_document_id, created_at_ms DESC)
    WHERE removed_at_ms IS NULL;

CREATE INDEX idx_document_relationships_target_active
    ON document_relationships(target_document_id, created_at_ms DESC)
    WHERE removed_at_ms IS NULL;
